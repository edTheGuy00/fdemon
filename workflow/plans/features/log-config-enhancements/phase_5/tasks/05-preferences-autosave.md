# Task: User Preferences Auto-save

**Objective**: Add functions to save and load user's last selected configuration and device to `settings.local.toml` for faster subsequent launches.

**Depends on**: None (independent)

## Scope

- `src/config/settings.rs` — Add save/load functions for last selection
- `src/config/types.rs` — Ensure `UserPreferences` has required fields

## Details

### UserPreferences Fields

The `UserPreferences` struct in `types.rs` already has:
- `last_device: Option<String>` — Last selected device ID
- `last_config: Option<String>` — Last selected config name

Verify these exist and add if missing.

### New Functions

Add to `src/config/settings.rs`:

```rust
/// Result of loading last selection
#[derive(Debug, Clone)]
pub struct LastSelection {
    pub config_name: Option<String>,
    pub device_id: Option<String>,
}

/// Load the user's last selection from settings.local.toml
///
/// Returns None if file doesn't exist or fields are not set.
pub fn load_last_selection(project_path: &Path) -> Option<LastSelection> {
    let prefs = load_user_preferences(project_path)?;

    // Only return if at least one field is set
    if prefs.last_config.is_none() && prefs.last_device.is_none() {
        return None;
    }

    Some(LastSelection {
        config_name: prefs.last_config,
        device_id: prefs.last_device,
    })
}

/// Save the user's selection to settings.local.toml
///
/// Preserves other preferences in the file.
pub fn save_last_selection(
    project_path: &Path,
    config_name: Option<&str>,
    device_id: Option<&str>,
) -> Result<()> {
    // Load existing preferences or create new
    let mut prefs = load_user_preferences(project_path).unwrap_or_default();

    // Update selection fields
    prefs.last_config = config_name.map(|s| s.to_string());
    prefs.last_device = device_id.map(|s| s.to_string());

    // Save back
    save_user_preferences(project_path, &prefs)
}

/// Clear the last selection (e.g., when user explicitly cancels)
pub fn clear_last_selection(project_path: &Path) -> Result<()> {
    if let Some(mut prefs) = load_user_preferences(project_path) {
        prefs.last_config = None;
        prefs.last_device = None;
        save_user_preferences(project_path, &prefs)
    } else {
        Ok(()) // Nothing to clear
    }
}

/// Check if last selection matches available configs and devices
///
/// Returns validated selection with indices, or None if not found.
pub fn validate_last_selection(
    selection: &LastSelection,
    configs: &LoadedConfigs,
    devices: &[Device],
) -> Option<ValidatedSelection> {
    let config_idx = selection.config_name.as_ref().and_then(|name| {
        configs
            .configs
            .iter()
            .position(|c| c.config.name == *name)
    });

    let device_idx = selection.device_id.as_ref().and_then(|id| {
        devices.iter().position(|d| d.id == *id)
    });

    // Return only if device is valid (config is optional)
    if device_idx.is_some() {
        Some(ValidatedSelection {
            config_idx,
            device_idx,
        })
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ValidatedSelection {
    pub config_idx: Option<usize>,
    pub device_idx: Option<usize>,
}
```

### Integration Points

These functions will be used by:
1. **Startup Flow** (Task 06) - Load last selection when auto_start=true
2. **Startup Dialog Confirm** (Task 06) - Save selection after launch
3. **Settings Panel** - Could display/clear in User tab

### Module Export

Update `src/config/mod.rs`:

```rust
pub use settings::{
    load_last_selection, save_last_selection, clear_last_selection,
    validate_last_selection, LastSelection, ValidatedSelection,
};
```

## Acceptance Criteria

1. `load_last_selection()` returns None if file missing or fields empty
2. `save_last_selection()` preserves other preferences
3. `save_last_selection()` creates file if missing
4. `clear_last_selection()` removes selection but keeps other prefs
5. `validate_last_selection()` returns indices only if device found
6. File format compatible with existing `settings.local.toml`

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_last_selection_missing_file() {
        let temp = tempdir().unwrap();
        let result = load_last_selection(temp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_last_selection() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        save_last_selection(
            temp.path(),
            Some("Debug"),
            Some("iPhone-15"),
        ).unwrap();

        let selection = load_last_selection(temp.path()).unwrap();
        assert_eq!(selection.config_name, Some("Debug".to_string()));
        assert_eq!(selection.device_id, Some("iPhone-15".to_string()));
    }

    #[test]
    fn test_save_preserves_other_prefs() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Save initial prefs with theme
        let mut prefs = UserPreferences::default();
        prefs.theme = Some("dark".to_string());
        save_user_preferences(temp.path(), &prefs).unwrap();

        // Save selection
        save_last_selection(temp.path(), Some("Debug"), None).unwrap();

        // Verify theme preserved
        let loaded = load_user_preferences(temp.path()).unwrap();
        assert_eq!(loaded.theme, Some("dark".to_string()));
        assert_eq!(loaded.last_config, Some("Debug".to_string()));
    }

    #[test]
    fn test_clear_last_selection() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Save selection
        save_last_selection(temp.path(), Some("Debug"), Some("device-1")).unwrap();

        // Clear it
        clear_last_selection(temp.path()).unwrap();

        // Verify cleared
        let selection = load_last_selection(temp.path());
        assert!(selection.is_none());
    }

    #[test]
    fn test_validate_last_selection() {
        let selection = LastSelection {
            config_name: Some("Debug".to_string()),
            device_id: Some("iphone-15".to_string()),
        };

        let configs = LoadedConfigs {
            configs: vec![
                SourcedConfig {
                    config: LaunchConfig { name: "Debug".to_string(), ..Default::default() },
                    source: ConfigSource::FDemon,
                    display_name: "Debug".to_string(),
                },
            ],
            vscode_start_index: None,
            is_empty: false,
        };

        let devices = vec![
            Device {
                id: "iphone-15".to_string(),
                name: "iPhone 15".to_string(),
                platform: "ios".to_string(),
                ..Default::default()
            },
        ];

        let validated = validate_last_selection(&selection, &configs, &devices).unwrap();
        assert_eq!(validated.config_idx, Some(0));
        assert_eq!(validated.device_idx, Some(0));
    }

    #[test]
    fn test_validate_requires_device() {
        let selection = LastSelection {
            config_name: Some("Debug".to_string()),
            device_id: Some("missing-device".to_string()),
        };

        let configs = LoadedConfigs::default();
        let devices: Vec<Device> = vec![];

        let validated = validate_last_selection(&selection, &configs, &devices);
        assert!(validated.is_none());
    }

    #[test]
    fn test_load_returns_none_if_fields_empty() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Save prefs without selection
        let prefs = UserPreferences::default();
        save_user_preferences(temp.path(), &prefs).unwrap();

        let selection = load_last_selection(temp.path());
        assert!(selection.is_none());
    }
}
```

## Notes

- `UserPreferences` fields already exist from Phase 4
- Auto-save is seamless - user doesn't need to confirm
- Selection cleared if device no longer available
- Config is optional (can launch without config using bare flutter run)

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**
(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy -- -D warnings` - Pending
- `cargo test selection` - Pending
