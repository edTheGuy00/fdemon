//! Configuration file parsing for Flutter Demon
//!
//! Supports:
//! - `.fdemon/config.toml` - Global settings
//! - `.fdemon/launch.toml` - Launch configurations
//! - `.vscode/launch.json` - VSCode compatibility

pub mod launch;
pub mod settings;
pub mod types;
pub mod vscode;

pub use launch::{
    find_config_by_name, get_auto_start_configs, init_launch_file, load_launch_configs,
};
pub use settings::{
    detect_editor, detect_parent_ide, editor_config_for_ide, find_editor_config, init_config_dir,
    load_settings, load_user_preferences, merge_preferences, save_user_preferences, EditorConfig,
    KNOWN_EDITORS,
};
pub use types::*;
pub use vscode::load_vscode_configs;
