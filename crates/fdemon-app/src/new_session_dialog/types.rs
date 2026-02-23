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
    EntryPoint,
    DartDefines,
    Launch,
}

impl LaunchContextField {
    pub fn next(self) -> Self {
        match self {
            Self::Config => Self::Mode,
            Self::Mode => Self::Flavor,
            Self::Flavor => Self::EntryPoint,
            Self::EntryPoint => Self::DartDefines,
            Self::DartDefines => Self::Launch,
            Self::Launch => Self::Config,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Config => Self::Launch,
            Self::Mode => Self::Config,
            Self::Flavor => Self::Mode,
            Self::EntryPoint => Self::Flavor,
            Self::DartDefines => Self::EntryPoint,
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
    /// Entry point selection (discovered Dart files with main())
    EntryPoint,
    /// Extra args editing (for settings panel extra args picker)
    ExtraArgs,
}

impl FuzzyModalType {
    /// Get the modal title
    pub fn title(&self) -> &'static str {
        match self {
            Self::Config => "Select Configuration",
            Self::Flavor => "Select Flavor",
            Self::EntryPoint => "Select Entry Point",
            Self::ExtraArgs => "Edit Extra Args",
        }
    }

    /// Whether custom input is allowed
    pub fn allows_custom(&self) -> bool {
        match self {
            Self::Config => false,    // Must select from list
            Self::Flavor => true,     // Can type custom flavor
            Self::EntryPoint => true, // Can type custom path
            Self::ExtraArgs => true,  // Users can type arbitrary args
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launch_context_field_next_includes_entry_point() {
        assert_eq!(
            LaunchContextField::Flavor.next(),
            LaunchContextField::EntryPoint
        );
        assert_eq!(
            LaunchContextField::EntryPoint.next(),
            LaunchContextField::DartDefines
        );
    }

    #[test]
    fn test_launch_context_field_prev_includes_entry_point() {
        assert_eq!(
            LaunchContextField::DartDefines.prev(),
            LaunchContextField::EntryPoint
        );
        assert_eq!(
            LaunchContextField::EntryPoint.prev(),
            LaunchContextField::Flavor
        );
    }

    #[test]
    fn test_launch_context_field_navigation_cycle() {
        // Forward cycle
        let mut field = LaunchContextField::Config;
        let fields = [
            LaunchContextField::Config,
            LaunchContextField::Mode,
            LaunchContextField::Flavor,
            LaunchContextField::EntryPoint,
            LaunchContextField::DartDefines,
            LaunchContextField::Launch,
        ];

        for expected in &fields[1..] {
            field = field.next();
            assert_eq!(field, *expected);
        }

        // Wraps around
        assert_eq!(field.next(), LaunchContextField::Config);
    }

    #[test]
    fn test_launch_context_field_next_enabled_skips_disabled() {
        // Simulate EntryPoint being disabled
        let is_disabled = |f: LaunchContextField| f == LaunchContextField::EntryPoint;

        let next = LaunchContextField::Flavor.next_enabled(is_disabled);
        assert_eq!(next, LaunchContextField::DartDefines);

        let prev = LaunchContextField::DartDefines.prev_enabled(is_disabled);
        assert_eq!(prev, LaunchContextField::Flavor);
    }

    #[test]
    fn test_fuzzy_modal_type_entry_point_title() {
        assert_eq!(FuzzyModalType::EntryPoint.title(), "Select Entry Point");
    }

    #[test]
    fn test_fuzzy_modal_type_entry_point_allows_custom() {
        // EntryPoint should allow custom input for typing arbitrary paths
        assert!(FuzzyModalType::EntryPoint.allows_custom());

        // Verify other types for consistency
        assert!(!FuzzyModalType::Config.allows_custom());
        assert!(FuzzyModalType::Flavor.allows_custom());
    }

    #[test]
    fn test_extra_args_fuzzy_modal_type() {
        assert_eq!(FuzzyModalType::ExtraArgs.title(), "Edit Extra Args");
        assert!(FuzzyModalType::ExtraArgs.allows_custom());
    }
}
