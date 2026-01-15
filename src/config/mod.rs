//! Configuration file parsing for Flutter Demon
//!
//! Supports:
//! - `.fdemon/config.toml` - Global settings
//! - `.fdemon/launch.toml` - Launch configurations
//! - `.vscode/launch.json` - VSCode compatibility

pub mod launch;
pub mod priority;
pub mod settings;
pub mod types;
pub mod vscode;
pub mod writer;

pub use launch::{
    add_launch_config, create_default_launch_config, delete_launch_config, find_config_by_name,
    get_auto_start_configs, init_launch_file, load_launch_configs, parse_dart_defines,
    save_launch_configs, update_launch_config_dart_defines, update_launch_config_field,
};
pub use priority::{
    find_config, get_first_auto_start, get_first_config, load_all_configs, LoadedConfigs,
    SourcedConfig,
};
pub use settings::{
    clear_last_selection, detect_editor, detect_parent_ide, editor_config_for_ide,
    find_editor_config, init_config_dir, init_fdemon_directory, load_last_selection, load_settings,
    load_user_preferences, merge_preferences, save_last_selection, save_settings,
    save_user_preferences, validate_last_selection, EditorConfig, LastSelection,
    ValidatedSelection, KNOWN_EDITORS,
};
pub use types::*;
pub use vscode::load_vscode_configs;
pub use writer::{
    save_fdemon_configs, update_config_dart_defines, update_config_flavor, update_config_mode,
    ConfigAutoSaver,
};
