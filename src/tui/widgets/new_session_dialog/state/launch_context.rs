//! Launch context state management

use super::dart_defines::DartDefine;
use super::types::LaunchContextField;
use crate::config::{ConfigSource, FlutterMode, LoadedConfigs};

/// State for the Launch Context pane
#[derive(Debug, Clone)]
pub struct LaunchContextState {
    /// Available configurations
    pub configs: LoadedConfigs,

    /// Index of selected configuration (None = no config, use defaults)
    pub selected_config_index: Option<usize>,

    /// Selected Flutter mode
    pub mode: FlutterMode,

    /// Flavor (from config or user override)
    pub flavor: Option<String>,

    /// Dart defines (from config or user override)
    pub dart_defines: Vec<DartDefine>,

    /// Currently focused field
    pub focused_field: LaunchContextField,
}

impl LaunchContextState {
    pub fn new(configs: LoadedConfigs) -> Self {
        Self {
            configs,
            selected_config_index: None,
            mode: FlutterMode::Debug,
            flavor: None,
            dart_defines: Vec::new(),
            focused_field: LaunchContextField::Config,
        }
    }

    /// Get the currently selected config
    pub fn selected_config(&self) -> Option<&crate::config::SourcedConfig> {
        self.selected_config_index
            .and_then(|i| self.configs.configs.get(i))
    }

    /// Get the source of the selected config
    pub fn selected_config_source(&self) -> Option<ConfigSource> {
        self.selected_config().map(|c| c.source)
    }

    /// Check if a field is editable based on config source
    pub fn is_field_editable(&self, field: LaunchContextField) -> bool {
        match field {
            // Config is always selectable
            LaunchContextField::Config => true,
            // Launch button is always enabled
            LaunchContextField::Launch => true,
            // Other fields depend on config source
            _ => {
                match self.selected_config_source() {
                    // VSCode configs: all fields read-only
                    Some(ConfigSource::VSCode) => false,
                    // FDemon configs: all fields editable
                    Some(ConfigSource::FDemon) => true,
                    // No config: all fields editable (transient)
                    None => true,
                    // CommandLine and Default configs: editable
                    Some(ConfigSource::CommandLine) | Some(ConfigSource::Default) => true,
                }
            }
        }
    }

    /// Check if mode is editable
    pub fn is_mode_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::Mode)
    }

    /// Check if flavor is editable
    pub fn is_flavor_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::Flavor)
    }

    /// Check if dart defines are editable
    pub fn are_dart_defines_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::DartDefines)
    }

    /// Select a configuration by index
    pub fn select_config(&mut self, index: Option<usize>) {
        self.selected_config_index = index;

        // Apply config values
        // Clone the config to avoid borrow checker issues
        if let Some(config) = self.selected_config().cloned() {
            self.mode = config.config.mode;

            if let Some(ref flavor) = config.config.flavor {
                self.flavor = Some(flavor.clone());
            }

            if !config.config.dart_defines.is_empty() {
                self.dart_defines = config
                    .config
                    .dart_defines
                    .iter()
                    .map(|(k, v)| DartDefine::new(k, v))
                    .collect();
            }
        }
    }

    /// Select a configuration by name
    pub fn select_config_by_name(&mut self, name: &str) {
        let index = self
            .configs
            .configs
            .iter()
            .position(|c| c.display_name == name);
        self.select_config(index);
    }

    /// Set flavor
    pub fn set_flavor(&mut self, flavor: Option<String>) {
        if self.is_flavor_editable() {
            self.flavor = flavor;
        }
    }

    /// Set dart defines
    pub fn set_dart_defines(&mut self, defines: Vec<DartDefine>) {
        if self.are_dart_defines_editable() {
            self.dart_defines = defines;
        }
    }

    /// Get flavor display string
    pub fn flavor_display(&self) -> String {
        self.flavor.clone().unwrap_or_else(|| "(none)".to_string())
    }

    /// Get dart defines display string
    pub fn dart_defines_display(&self) -> String {
        let count = self.dart_defines.len();
        if count == 0 {
            "(none)".to_string()
        } else if count == 1 {
            "1 item".to_string()
        } else {
            format!("{} items", count)
        }
    }

    /// Get config display string
    pub fn config_display(&self) -> String {
        self.selected_config()
            .map(|c| c.display_name.clone())
            .unwrap_or_else(|| "(none)".to_string())
    }
}
