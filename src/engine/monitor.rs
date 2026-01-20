use anyhow::{Context, Result};
use evdev::{Device, EventType, InputEventKind, Key};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::config::Config;
use crate::engine::keymaps::KeyMap;

/// Events emitted by the keyboard monitor
#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    /// A character was typed
    Character(char),
    /// Backspace was pressed
    Backspace,
    /// A word boundary character was typed (space, punctuation, etc.)
    WordBoundary(char),
    /// Enter/Return was pressed
    Enter,
    /// Tab was pressed
    Tab,
    /// Escape was pressed
    Escape,
}

/// Keyboard monitor that reads from evdev devices
pub struct KeyboardMonitor {
    devices: Vec<Device>,
    event_tx: mpsc::Sender<KeyboardEvent>,
    config: Arc<RwLock<Config>>,
}

impl KeyboardMonitor {
    /// Create a new keyboard monitor
    pub fn new(event_tx: mpsc::Sender<KeyboardEvent>, config: Arc<RwLock<Config>>) -> Result<Self> {
        let devices = Self::find_keyboard_devices()?;

        if devices.is_empty() {
            anyhow::bail!(
                "No keyboard devices found. Make sure you have permission to read from /dev/input/event* \
                 (add your user to the 'input' group: sudo usermod -aG input $USER)"
            );
        }

        log::info!("Found {} keyboard device(s)", devices.len());
        for device in &devices {
            if let Some(name) = device.name() {
                log::debug!("  - {}", name);
            }
        }

        Ok(Self { devices, event_tx, config })
    }

    /// Find all keyboard devices in /dev/input/
    fn find_keyboard_devices() -> Result<Vec<Device>> {
        let mut keyboards = Vec::new();

        let input_dir = PathBuf::from("/dev/input");
        if !input_dir.exists() {
            anyhow::bail!("/dev/input directory not found");
        }

        for entry in std::fs::read_dir(&input_dir)
            .context("Failed to read /dev/input directory")?
        {
            let entry = entry?;
            let path = entry.path();

            // Only look at event* devices
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with("event") {
                    continue;
                }
            } else {
                continue;
            }

            // Try to open the device
            match Device::open(&path) {
                Ok(device) => {
                    // Check if this device has keyboard capabilities
                    if Self::is_keyboard(&device) {
                        log::debug!("Found keyboard: {} ({:?})",
                            device.name().unwrap_or("Unknown"), path);
                        keyboards.push(device);
                    }
                }
                Err(e) => {
                    log::trace!("Could not open {:?}: {}", path, e);
                }
            }
        }

        Ok(keyboards)
    }

    /// Check if a device is a keyboard (has key events for common keys)
    fn is_keyboard(device: &Device) -> bool {
        let Some(supported_keys) = device.supported_keys() else {
            return false;
        };

        // A keyboard should have letter keys
        let has_letters = supported_keys.contains(Key::KEY_A)
            && supported_keys.contains(Key::KEY_Z);

        // And some common keys
        let has_common = supported_keys.contains(Key::KEY_ENTER)
            && supported_keys.contains(Key::KEY_SPACE);

        has_letters && has_common
    }

    /// Start monitoring keyboard events
    pub async fn run(self) -> Result<()> {
        let mut shift_pressed = false;
        let mut caps_lock = false;

        // Dynamic layout handling
        let mut current_layout = String::new();
        // Initialize with default/empty, will be updated in loop
        let mut key_mapper = KeyMap::new("qwerty");

        // We need to poll all devices. For simplicity, we'll use blocking reads
        // in a separate thread and communicate via channels.

        let (internal_tx, mut internal_rx) = mpsc::channel::<(Key, i32)>(256);

        // Spawn blocking threads for each device
        for device in self.devices {
            let tx = internal_tx.clone();
            std::thread::spawn(move || {
                Self::device_reader(device, tx);
            });
        }

        // Drop the original sender so the channel closes when all devices are done
        drop(internal_tx);

        // Process events
        while let Some((key, value)) = internal_rx.recv().await {
            // Check for layout change
            {
                let config = self.config.read().await;
                if config.settings.layout != current_layout {
                    current_layout = config.settings.layout.clone();
                    key_mapper = KeyMap::new(&current_layout);
                    log::info!("Keyboard layout switched to: {}", current_layout);
                }
            }

            // value: 0 = release, 1 = press, 2 = repeat
            let is_press = value == 1;
            let is_release = value == 0;

            // Track modifier states
            match key {
                Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => {
                    shift_pressed = is_press;
                    continue;
                }
                Key::KEY_CAPSLOCK if is_press => {
                    caps_lock = !caps_lock;
                    continue;
                }
                _ => {}
            }

            // Only process key presses (not releases or repeats for most keys)
            if !is_press {
                // Allow backspace repeat
                if key != Key::KEY_BACKSPACE || is_release {
                    continue;
                }
            }

            log::debug!("Key press detected: {:?}", key);


            let event = match key {
                Key::KEY_BACKSPACE => Some(KeyboardEvent::Backspace),
                Key::KEY_ENTER | Key::KEY_KPENTER => Some(KeyboardEvent::Enter),
                Key::KEY_TAB => Some(KeyboardEvent::Tab),
                Key::KEY_ESC => Some(KeyboardEvent::Escape),
                _ => {
                    if let Some(ch) = key_mapper.map_key(key, shift_pressed, caps_lock) {
                        if ch == ' ' || ch.is_ascii_punctuation() {
                            Some(KeyboardEvent::WordBoundary(ch))
                        } else {
                            Some(KeyboardEvent::Character(ch))
                        }
                    } else {
                        None
                    }
                }
            };

            if let Some(event) = event {
                if self.event_tx.send(event).await.is_err() {
                    log::debug!("Event receiver dropped, stopping monitor");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Read events from a single device (runs in blocking thread)
    fn device_reader(mut device: Device, tx: mpsc::Sender<(Key, i32)>) {
        loop {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        if event.event_type() == EventType::KEY {
                            if let InputEventKind::Key(key) = event.kind() {
                                if tx.blocking_send((key, event.value())).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error reading from device: {}", e);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_mapper() {
        let mapper = KeyMap::new("qwerty");

        // Test letters
        assert_eq!(mapper.map_key(Key::KEY_A, false, false), Some('a'));
        assert_eq!(mapper.map_key(Key::KEY_A, true, false), Some('A'));
        assert_eq!(mapper.map_key(Key::KEY_A, false, true), Some('A')); // caps lock
        assert_eq!(mapper.map_key(Key::KEY_A, true, true), Some('a')); // shift + caps

        // Test numbers
        assert_eq!(mapper.map_key(Key::KEY_1, false, false), Some('1'));
        assert_eq!(mapper.map_key(Key::KEY_1, true, false), Some('!'));

        // Test punctuation
        assert_eq!(mapper.map_key(Key::KEY_SEMICOLON, false, false), Some(';'));
        assert_eq!(mapper.map_key(Key::KEY_SEMICOLON, true, false), Some(':'));
    }
}
