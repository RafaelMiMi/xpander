use anyhow::Result;
use ksni::{self, menu::StandardItem, Icon, MenuItem, Tray, TrayService};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;

/// Commands that can be sent from the tray menu
#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// Toggle the expansion engine on/off
    ToggleEnabled,
    /// Open the configuration window
    OpenConfig,
    /// Open the config file in editor
    EditConfigFile,
    /// Reload configuration
    ReloadConfig,
    /// Quit the application
    Quit,
}

/// State shared with the tray icon
struct TrayState {
    enabled: bool,
    command_tx: mpsc::Sender<TrayCommand>,
}

/// The system tray implementation
struct XpanderTray {
    state: Arc<RwLock<TrayState>>,
}

impl Tray for XpanderTray {
    fn id(&self) -> String {
        "xpander".to_string()
    }

    fn title(&self) -> String {
        "Xpander".to_string()
    }

    fn icon_name(&self) -> String {
        // Use a standard icon that's likely to be available
        "input-keyboard".to_string()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        // Provide a fallback icon as ARGB pixel data
        // This is a simple 22x22 keyboard-like icon
        let size = 22;
        let mut data = Vec::with_capacity(size * size * 4);

        for y in 0..size {
            for x in 0..size {
                let (r, g, b, a) = if x >= 2 && x < size - 2 && y >= 6 && y < size - 4 {
                    // Main body - dark gray
                    (80, 80, 80, 255)
                } else if x >= 4 && x < size - 4 && y >= 8 && y < size - 6 {
                    // Keys area - lighter
                    (120, 120, 120, 255)
                } else {
                    // Transparent
                    (0, 0, 0, 0)
                };
                // ARGB format
                data.push(a);
                data.push(r);
                data.push(g);
                data.push(b);
            }
        }

        vec![Icon {
            width: size as i32,
            height: size as i32,
            data,
        }]
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        // Get current enabled state using std RwLock (non-async)
        let enabled = self.state.read().map(|s| s.enabled).unwrap_or(true);

        vec![
            MenuItem::Standard(StandardItem {
                label: if enabled {
                    "Disable Expansions".to_string()
                } else {
                    "Enable Expansions".to_string()
                },
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_tx.try_send(TrayCommand::ToggleEnabled);
                    }
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Open Configuration...".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_tx.try_send(TrayCommand::OpenConfig);
                    }
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: "Edit Config File".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_tx.try_send(TrayCommand::EditConfigFile);
                    }
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: "Reload Configuration".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_tx.try_send(TrayCommand::ReloadConfig);
                    }
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Quit".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    if let Ok(state) = tray.state.read() {
                        let _ = state.command_tx.try_send(TrayCommand::Quit);
                    }
                }),
                ..Default::default()
            }),
        ]
    }
}

/// Handle for controlling the system tray
pub struct TrayHandle {
    state: Arc<RwLock<TrayState>>,
}

impl TrayHandle {
    /// Update the enabled state (will be reflected in menu)
    pub fn set_enabled(&self, enabled: bool) {
        if let Ok(mut state) = self.state.write() {
            state.enabled = enabled;
        }
    }
}

/// Start the system tray icon
pub fn start_tray(
    enabled: bool,
    command_tx: mpsc::Sender<TrayCommand>,
) -> Result<TrayHandle> {
    let state = Arc::new(RwLock::new(TrayState {
        enabled,
        command_tx,
    }));

    let tray = XpanderTray {
        state: state.clone(),
    };

    let service = TrayService::new(tray);
    service.spawn();

    log::info!("System tray started");

    Ok(TrayHandle {
        state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_state() {
        let (tx, _rx) = mpsc::channel(10);
        let state = Arc::new(RwLock::new(TrayState {
            enabled: true,
            command_tx: tx,
        }));

        assert!(state.read().unwrap().enabled);

        state.write().unwrap().enabled = false;
        assert!(!state.read().unwrap().enabled);
    }
}
