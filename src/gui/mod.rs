pub mod editor;
pub mod tray;
pub mod window;

pub use editor::SnippetEditor;
pub use tray::{start_tray, TrayCommand, TrayHandle};
pub use window::{create_config_app, ConfigWindow};
