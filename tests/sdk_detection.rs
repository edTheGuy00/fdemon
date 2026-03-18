//! SDK detection integration tests.
//!
//! Tests for the multi-strategy Flutter SDK detection introduced in Phase 1.
//!
//! Run with: `cargo test --test sdk_detection`

mod sdk_detection {
    pub mod assertions;
    pub mod docker_helpers;
    pub mod fixtures;
    pub mod tier1_detection_chain;
    pub mod tier1_edge_cases;
    pub mod tier2_headless;
    pub mod tier2_linux;
    pub mod tier2_windows;
}

// Re-export at the crate root so test sub-modules can be loaded independently.
use sdk_detection::{assertions, fixtures};

// ─────────────────────────────────────────────────────────────────────────────
// Self-tests: verify the fixture builders work against the real detection
// functions.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::assertions::{
        assert_sdk_not_found, assert_sdk_root, assert_sdk_source, parse_headless_events,
    };
    use super::fixtures::{
        create_asdf_layout, create_flutter_project, create_flutter_wrapper_layout,
        create_fvm_layout, create_fvm_legacy_layout, create_mise_layout, create_proto_layout,
        create_puro_layout, EnvGuard, MockSdkBuilder,
    };
    use fdemon_daemon::flutter_sdk::{find_flutter_sdk, validate_sdk_path, SdkSource};
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    // ── MockSdkBuilder ────────────────────────────────────────────────────────

    #[test]
    fn test_mock_sdk_passes_validation() {
        let tmp = TempDir::new().unwrap();
        let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0")
            .with_dart_sdk()
            .build();
        assert!(
            validate_sdk_path(&sdk_root).is_ok(),
            "MockSdkBuilder should produce a valid SDK that passes validate_sdk_path()"
        );
    }

    #[test]
    fn test_mock_sdk_without_dart_sdk_still_passes_validation() {
        // validate_sdk_path does not require bin/cache/dart-sdk/ (fresh installs are allowed)
        let tmp = TempDir::new().unwrap();
        let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0").build();
        assert!(
            validate_sdk_path(&sdk_root).is_ok(),
            "MockSdkBuilder without dart-sdk should still pass validate_sdk_path()"
        );
    }

    #[test]
    fn test_mock_sdk_version_readable() {
        let tmp = TempDir::new().unwrap();
        let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0").build();
        let version = fdemon_daemon::flutter_sdk::read_version_file(&sdk_root).unwrap();
        assert_eq!(version, "3.22.0");
    }

    // ── FVM modern fixture ────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_fvm_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        let cache = tmp.path().join("fvm_cache");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_fvm_layout(&project, &cache, "3.22.0");

        // Isolate FLUTTER_ROOT so it cannot interfere with priority ordering.
        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
        let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", cache.to_str().unwrap());

        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(
            &sdk,
            &SdkSource::Fvm {
                version: "3.22.0".into(),
            },
        );
        assert_sdk_root(&sdk, &sdk_root);
    }

    // ── FVM legacy fixture ────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_fvm_legacy_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        let cache = tmp.path().join("fvm_cache");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_fvm_legacy_layout(&project, &cache, "3.22.0");

        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
        let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", cache.to_str().unwrap());

        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(
            &sdk,
            &SdkSource::Fvm {
                version: "3.22.0".into(),
            },
        );
        // Legacy: symlink is resolved so canonicalize the expected path too.
        let canonical_sdk_root = fs::canonicalize(&sdk_root).unwrap_or(sdk_root);
        assert_sdk_root(&sdk, &canonical_sdk_root);
    }

    // ── Puro fixture ──────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_puro_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        let puro_root = tmp.path().join("puro_root");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_puro_layout(&project, &puro_root, "default");

        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
        let _puro_guard = EnvGuard::set("PURO_ROOT", puro_root.to_str().unwrap());

        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(
            &sdk,
            &SdkSource::Puro {
                env: "default".into(),
            },
        );
        assert_sdk_root(&sdk, &sdk_root);
    }

    // ── asdf fixture ──────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_asdf_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        let asdf_data = tmp.path().join("asdf_data");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_asdf_layout(&project, &asdf_data, "3.22.0");

        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
        let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());

        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(
            &sdk,
            &SdkSource::Asdf {
                version: "3.22.0".into(),
            },
        );
        assert_sdk_root(&sdk, &sdk_root);
    }

    // ── mise fixture ──────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_mise_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        let mise_data = tmp.path().join("mise_data");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_mise_layout(&project, &mise_data, "3.22.0");

        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
        let _mise_guard = EnvGuard::set("MISE_DATA_DIR", mise_data.to_str().unwrap());

        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(
            &sdk,
            &SdkSource::Mise {
                version: "3.22.0".into(),
            },
        );
        assert_sdk_root(&sdk, &sdk_root);
    }

    // ── proto fixture ─────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_proto_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        let proto_home = tmp.path().join("proto_home");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_proto_layout(&project, &proto_home, "3.22.0");

        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
        let _proto_guard = EnvGuard::set("PROTO_HOME", proto_home.to_str().unwrap());

        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(
            &sdk,
            &SdkSource::Proto {
                version: "3.22.0".into(),
            },
        );
        assert_sdk_root(&sdk, &sdk_root);
    }

    // ── flutter_wrapper fixture ───────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_flutter_wrapper_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_flutter_wrapper_layout(&project);

        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");

        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(&sdk, &SdkSource::FlutterWrapper);
        assert_sdk_root(&sdk, &sdk_root);
    }

    // ── EnvGuard ──────────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_env_guard_set_restores_on_drop() {
        let key = "FDEMON_TEST_ENV_GUARD_SET";
        std::env::remove_var(key); // ensure clean state

        {
            let _guard = EnvGuard::set(key, "test_value");
            assert_eq!(std::env::var(key).unwrap(), "test_value");
        }
        // After guard drops the variable must be gone (it didn't exist before)
        assert!(
            std::env::var(key).is_err(),
            "variable should be removed after guard drops"
        );
    }

    #[test]
    #[serial]
    fn test_env_guard_set_restores_previous_value() {
        let key = "FDEMON_TEST_ENV_GUARD_PREV";
        std::env::set_var(key, "original");

        {
            let _guard = EnvGuard::set(key, "overridden");
            assert_eq!(std::env::var(key).unwrap(), "overridden");
        }
        assert_eq!(std::env::var(key).unwrap(), "original");
        std::env::remove_var(key); // cleanup
    }

    #[test]
    #[serial]
    fn test_env_guard_remove_restores_on_drop() {
        let key = "FDEMON_TEST_ENV_GUARD_REMOVE";
        std::env::set_var(key, "was_here");

        {
            let _guard = EnvGuard::remove(key);
            assert!(
                std::env::var(key).is_err(),
                "variable should be absent inside guard"
            );
        }
        assert_eq!(std::env::var(key).unwrap(), "was_here");
        std::env::remove_var(key); // cleanup
    }

    #[test]
    #[serial]
    fn test_env_guard_remove_noop_when_absent() {
        let key = "FDEMON_TEST_ENV_GUARD_ABSENT";
        std::env::remove_var(key);

        {
            let _guard = EnvGuard::remove(key);
            assert!(std::env::var(key).is_err());
        }
        // Should still not exist after drop
        assert!(std::env::var(key).is_err());
    }

    // ── assert_sdk_not_found ──────────────────────────────────────────────────

    #[test]
    #[serial]
    fn test_assert_sdk_not_found_passes_on_flutter_not_found() {
        let tmp = TempDir::new().unwrap();
        // Isolate PATH and env vars so no flutter binary is found anywhere.
        let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());
        let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");

        let result = find_flutter_sdk(tmp.path(), None);
        assert_sdk_not_found(&result);
    }

    // ── parse_headless_events ─────────────────────────────────────────────────

    #[test]
    fn test_parse_headless_events_single_line() {
        let output = r#"{"event":"daemon_connected","timestamp":"2024-01-01T00:00:00Z"}"#;
        let events = parse_headless_events(output);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "daemon_connected");
    }

    #[test]
    fn test_parse_headless_events_multiple_lines() {
        let output = concat!(
            r#"{"event":"daemon_connected","timestamp":"2024-01-01T00:00:00Z"}"#,
            "\n",
            r#"{"event":"app_started","app_id":"abc","timestamp":"2024-01-01T00:00:01Z"}"#,
            "\n",
            r#"{"event":"log","message":"Hello","timestamp":"2024-01-01T00:00:02Z"}"#,
        );
        let events = parse_headless_events(output);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event, "daemon_connected");
        assert_eq!(events[1].event, "app_started");
        assert_eq!(events[2].event, "log");
    }

    #[test]
    fn test_parse_headless_events_skips_blank_lines() {
        let output = concat!(
            r#"{"event":"daemon_connected","timestamp":"2024-01-01T00:00:00Z"}"#,
            "\n\n",
            r#"{"event":"app_started","timestamp":"2024-01-01T00:00:01Z"}"#,
            "\n",
        );
        let events = parse_headless_events(output);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_parse_headless_events_error_event_captures_fields() {
        let output = r#"{"event":"error","message":"SDK not found","fatal":true,"timestamp":"2024-01-01T00:00:00Z"}"#;
        let events = parse_headless_events(output);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "error");
        assert_eq!(events[0].message.as_deref(), Some("SDK not found"));
        assert_eq!(events[0].fatal, Some(true));
    }

    #[test]
    fn test_parse_headless_events_empty_input() {
        let events = parse_headless_events("");
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_headless_events_non_json_lines_skipped() {
        let output = concat!(
            "not json at all\n",
            r#"{"event":"app_started","timestamp":"2024-01-01T00:00:00Z"}"#,
            "\n",
        );
        let events = parse_headless_events(output);
        // The non-JSON line is silently skipped; only the valid event is returned.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "app_started");
    }
}
