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
pub use settings::{init_config_dir, load_settings};
pub use types::*;
pub use vscode::load_vscode_configs;
