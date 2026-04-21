pub mod config;
pub mod hotkey;
pub mod state;
pub mod websocket;

pub use config::{ConfigPaths, ensure_themes_dir_exists, create_default_theme_if_needed, get_themes, swap_theme};
pub use state::AppState;
pub use websocket::GlazeSocket;
