mod config;
mod engine;
mod gui;
mod variables;

use anyhow::{Context, Result};
use std::env;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use config::{Config, ConfigManager};
use engine::start_expansion_pipeline;
use gui::{start_tray, TrayCommand, create_config_app};

/// Application state shared across components
struct AppState {
    config: Arc<RwLock<Config>>,
    enabled: Arc<RwLock<bool>>,
    config_manager: Arc<RwLock<ConfigManager>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    )
    .format_timestamp_secs()
    .init();

    // Check for --gui flag to open config window
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--gui" || a == "-g") {
        return run_config_gui().await;
    }

    log::info!("Starting xpander text expansion daemon");

    // Check prerequisites
    check_prerequisites().await?;

    // Load configuration
    let (config_manager, mut config_rx) = ConfigManager::new()
        .await
        .context("Failed to initialize configuration")?;

    let initial_config = config_manager.get_config().await.clone();
    let initial_enabled = initial_config.settings.enabled;

    log::info!(
        "Loaded {} snippets from {}",
        initial_config.snippets.len(),
        config_manager.path().display()
    );

    // Create shared state
    let config = Arc::new(RwLock::new(initial_config));
    let enabled = Arc::new(RwLock::new(initial_enabled));
    let config_manager = Arc::new(RwLock::new(config_manager));

    let state = AppState {
        config: config.clone(),
        enabled: enabled.clone(),
        config_manager: config_manager.clone(),
    };

    // Create channel for tray commands
    let (tray_tx, mut tray_rx) = mpsc::channel::<TrayCommand>(32);

    // Start system tray
    let tray_handle = start_tray(initial_enabled, tray_tx)
        .context("Failed to start system tray")?;

    // Handle config reload notifications
    let config_for_reload = config.clone();
    tokio::spawn(async move {
        while let Some(new_config) = config_rx.recv().await {
            let mut cfg = config_for_reload.write().await;
            *cfg = new_config;
            log::info!("Configuration reloaded");
        }
    });

    // Handle tray commands
    let state_for_tray = Arc::new(state);
    let tray_handle = Arc::new(tray_handle);

    let tray_handle_clone = tray_handle.clone();
    let state_clone = state_for_tray.clone();

    tokio::spawn(async move {
        while let Some(cmd) = tray_rx.recv().await {
            match cmd {
                TrayCommand::ToggleEnabled => {
                    let mut enabled = state_clone.enabled.write().await;
                    *enabled = !*enabled;
                    let new_state = *enabled;
                    drop(enabled);

                    tray_handle_clone.set_enabled(new_state);
                    log::info!("Expansions {}", if new_state { "enabled" } else { "disabled" });
                }
                TrayCommand::OpenConfig => {
                    log::info!("Opening configuration window");
                    // Spawn xpander with --gui flag to open config window
                    if let Ok(exe) = env::current_exe() {
                        if let Err(e) = Command::new(exe).arg("--gui").spawn() {
                            log::error!("Failed to open config window: {}", e);
                        }
                    }
                }
                TrayCommand::EditConfigFile => {
                    let config_path = {
                        let manager = state_clone.config_manager.read().await;
                        manager.path().to_path_buf()
                    };

                    log::info!("Opening config file: {}", config_path.display());

                    // Try to open with default editor
                    if let Err(e) = open_file_in_editor(&config_path) {
                        log::error!("Failed to open config file: {}", e);
                    }
                }
                TrayCommand::ReloadConfig => {
                    let manager = state_clone.config_manager.read().await;
                    match ConfigManager::load_config(manager.path()) {
                        Ok(new_config) => {
                            let mut cfg = state_clone.config.write().await;
                            *cfg = new_config;
                            log::info!("Configuration reloaded manually");
                        }
                        Err(e) => {
                            log::error!("Failed to reload config: {}", e);
                        }
                    }
                }
                TrayCommand::Quit => {
                    log::info!("Quit requested, shutting down");
                    std::process::exit(0);
                }
            }
        }
    });

    // Start the expansion pipeline
    log::info!("Starting expansion engine");
    start_expansion_pipeline(config, enabled).await?;

    Ok(())
}

/// Check that all prerequisites are met
async fn check_prerequisites() -> Result<()> {
    // Check for ydotool
    engine::OutputEngine::check_availability().await
        .context(
            "ydotool is required for text expansion on Wayland.\n\
             Install with: sudo apt install ydotool\n\
             Then enable the daemon: sudo systemctl enable --now ydotool"
        )?;

    // Check for input group membership
    check_input_group()?;

    Ok(())
}

/// Check if user is in the input group
fn check_input_group() -> Result<()> {
    let groups_output = Command::new("groups")
        .output()
        .context("Failed to check user groups")?;

    let groups = String::from_utf8_lossy(&groups_output.stdout);

    if !groups.contains("input") {
        log::warn!(
            "User may not be in 'input' group. If keyboard monitoring fails, run:\n\
             sudo usermod -aG input $USER\n\
             Then log out and back in."
        );
    }

    Ok(())
}

/// Open a file in the default editor
fn open_file_in_editor(path: &std::path::Path) -> Result<()> {
    // Try common editors in order of preference
    let editors = ["xdg-open", "gedit", "kate", "code", "vim"];

    for editor in editors {
        if Command::new("which")
            .arg(editor)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            Command::new(editor)
                .arg(path)
                .spawn()
                .context(format!("Failed to open with {}", editor))?;
            return Ok(());
        }
    }

    anyhow::bail!("No suitable editor found")
}

/// Run the GTK configuration GUI
async fn run_config_gui() -> Result<()> {
    use gtk4::prelude::*;

    let app = create_config_app();
    // Pass empty args so GTK doesn't try to parse our --gui flag
    let empty_args: &[&str] = &[];
    app.run_with_args(empty_args);

    Ok(())
}

/// Print usage information
#[allow(dead_code)]
fn print_usage() {
    eprintln!(
        r#"xpander - Text Expansion for Linux (Wayland)

USAGE:
    xpander [OPTIONS]

OPTIONS:
    -h, --help      Show this help message
    -v, --version   Show version information
    -c, --config    Path to config file (default: ~/.config/xpander/config.yaml)

PREREQUISITES:
    1. Install ydotool:
       sudo apt install ydotool
       sudo systemctl enable --now ydotool

    2. Add user to input group:
       sudo usermod -aG input $USER
       (Log out and back in for this to take effect)

CONFIGURATION:
    Edit ~/.config/xpander/config.yaml to add snippets:

    snippets:
      - trigger: ";email"
        replace: "myemail@example.com"

      - trigger: ";date"
        replace: "{{{{date:%Y-%m-%d}}}}"

VARIABLES:
    {{{{date}}}}         - Current date (YYYY-MM-DD)
    {{{{date:FORMAT}}}}  - Date with custom format
    {{{{time}}}}         - Current time (HH:MM:SS)
    {{{{datetime}}}}     - Date and time
    {{{{clipboard}}}}    - Clipboard contents
    {{{{random:N}}}}     - Random N-digit number
    {{{{env:VAR}}}}      - Environment variable
    {{{{shell:CMD}}}}    - Shell command output
    {{{{uuid}}}}         - Random UUID

For more information, see: https://github.com/example/xpander
"#
    );
}
