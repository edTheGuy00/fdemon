//! Core enums for NewSessionDialog state

use std::path::PathBuf;

/// Represents which pane of the NewSessionDialog has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogPane {
    #[default]
    TargetSelector,
    LaunchContext,
}

impl DialogPane {
    pub fn toggle(self) -> Self {
        match self {
            DialogPane::TargetSelector => DialogPane::LaunchContext,
            DialogPane::LaunchContext => DialogPane::TargetSelector,
        }
    }
}

/// Tabs in the Target Selector pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetTab {
    #[default]
    Connected, // Running/connected devices
    Bootable, // Offline simulators/AVDs
}

impl TargetTab {
    pub fn label(&self) -> &'static str {
        match self {
            TargetTab::Connected => "1 Connected",
            TargetTab::Bootable => "2 Bootable",
        }
    }

    pub fn shortcut(&self) -> char {
        match self {
            TargetTab::Connected => '1',
            TargetTab::Bootable => '2',
        }
    }

    /// Get the other tab
    pub fn toggle(&self) -> Self {
        match self {
            TargetTab::Connected => TargetTab::Bootable,
            TargetTab::Bootable => TargetTab::Connected,
        }
    }
}

/// Fields in the Launch Context pane for navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchContextField {
    #[default]
    Config,
    Mode,
    Flavor,
    DartDefines,
    Launch,
}

impl LaunchContextField {
    pub fn next(self) -> Self {
        match self {
            Self::Config => Self::Mode,
            Self::Mode => Self::Flavor,
            Self::Flavor => Self::DartDefines,
            Self::DartDefines => Self::Launch,
            Self::Launch => Self::Config,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Config => Self::Launch,
            Self::Mode => Self::Config,
            Self::Flavor => Self::Mode,
            Self::DartDefines => Self::Flavor,
            Self::Launch => Self::DartDefines,
        }
    }

    /// Skip disabled fields when navigating forward
    pub fn next_enabled(self, is_disabled: impl Fn(Self) -> bool) -> Self {
        let mut next = self.next();
        // Avoid infinite loop if all fields disabled
        let start = next;
        while is_disabled(next) && next.next() != start {
            next = next.next();
        }
        next
    }

    /// Skip disabled fields when navigating backward
    pub fn prev_enabled(self, is_disabled: impl Fn(Self) -> bool) -> Self {
        let mut prev = self.prev();
        let start = prev;
        while is_disabled(prev) && prev.prev() != start {
            prev = prev.prev();
        }
        prev
    }
}

/// Type of fuzzy modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyModalType {
    /// Configuration selection (from LoadedConfigs)
    Config,
    /// Flavor selection (from project + custom)
    Flavor,
}

impl FuzzyModalType {
    /// Get the modal title
    pub fn title(&self) -> &'static str {
        match self {
            Self::Config => "Select Configuration",
            Self::Flavor => "Select Flavor",
        }
    }

    /// Whether custom input is allowed
    pub fn allows_custom(&self) -> bool {
        match self {
            Self::Config => false, // Must select from list
            Self::Flavor => true,  // Can type custom flavor
        }
    }
}

/// A single dart define key-value pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartDefine {
    pub key: String,
    pub value: String,
}

impl DartDefine {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Format as command line argument
    pub fn to_arg(&self) -> String {
        format!("{}={}", self.key, self.value)
    }
}

/// Parameters for launching a Flutter session
#[derive(Debug, Clone)]
pub struct LaunchParams {
    pub device_id: String,
    pub mode: crate::config::FlutterMode,
    pub flavor: Option<String>,
    pub dart_defines: Vec<String>,
    pub config_name: Option<String>,
    pub entry_point: Option<PathBuf>,
}
