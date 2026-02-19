//! Tests for the session module.

#[cfg(test)]
mod tests {
    use fdemon_core::{performance::MemoryUsage, AppPhase, LogEntry, LogLevel, LogSource};

    use crate::config::LaunchConfig;
    use crate::session::{
        CollapseState, LogBatcher, PerformanceState, Session, SessionHandle,
        STATS_RECOMPUTE_INTERVAL,
    };

    // Import constants used in tests — access via the submodule paths
    use crate::session::log_batcher::{BATCH_FLUSH_INTERVAL, BATCH_MAX_SIZE};
    use crate::session::performance::DEFAULT_FRAME_HISTORY_SIZE;

    use fdemon_core::performance::{FrameTiming, RingBuffer};

    #[test]
    fn test_session_creation() {
        let session = Session::new(
            "device-123".to_string(),
            "iPhone 15 Pro".to_string(),
            "ios".to_string(),
            true,
        );

        assert_eq!(session.device_id, "device-123");
        assert_eq!(session.device_name, "iPhone 15 Pro");
        assert_eq!(session.name, "iPhone 15 Pro");
        assert!(session.is_emulator);
        assert_eq!(session.phase, AppPhase::Initializing);
        assert!(session.logs.is_empty());
    }

    #[test]
    fn test_session_id_uniqueness() {
        let s1 = Session::new("a".into(), "A".into(), "ios".into(), false);
        let s2 = Session::new("b".into(), "B".into(), "ios".into(), false);
        let s3 = Session::new("c".into(), "C".into(), "ios".into(), false);

        assert_ne!(s1.id, s2.id);
        assert_ne!(s2.id, s3.id);
        assert_ne!(s1.id, s3.id);
    }

    #[test]
    fn test_session_logging() {
        let mut session = Session::new("d".into(), "Device".into(), "android".into(), false);

        session.log_info(LogSource::App, "Test message");
        session.log_error(LogSource::Daemon, "Error message");

        assert_eq!(session.logs.len(), 2);
    }

    #[test]
    fn test_session_log_trimming() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        session.max_logs = 5;

        for i in 0..10 {
            session.log_info(LogSource::App, format!("Message {}", i));
        }

        assert_eq!(session.logs.len(), 5);
        // Should have messages 5-9
        assert!(session.logs[0].message.contains('5'));
        assert!(session.logs[4].message.contains('9'));
    }

    #[test]
    fn test_session_lifecycle() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert_eq!(session.phase, AppPhase::Initializing);
        assert!(session.app_id.is_none());

        session.mark_started("app-123".to_string());
        assert_eq!(session.phase, AppPhase::Running);
        assert_eq!(session.app_id, Some("app-123".to_string()));
        assert!(session.started_at.is_some());

        session.start_reload();
        assert_eq!(session.phase, AppPhase::Reloading);
        assert!(session.reload_start_time.is_some());

        session.complete_reload();
        assert_eq!(session.phase, AppPhase::Running);
        assert_eq!(session.reload_count, 1);
        assert!(session.last_reload_time.is_some());

        session.mark_stopped();
        assert_eq!(session.phase, AppPhase::Stopped);
    }

    #[test]
    fn test_session_status_icons() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert_eq!(session.status_icon(), "○"); // Initializing

        session.phase = AppPhase::Running;
        assert_eq!(session.status_icon(), "●");

        session.phase = AppPhase::Reloading;
        assert_eq!(session.status_icon(), "↻");

        session.phase = AppPhase::Stopped;
        assert_eq!(session.status_icon(), "○");
    }

    #[test]
    fn test_tab_title_truncation() {
        let short = Session::new("d".into(), "iPhone".into(), "ios".into(), false);
        assert_eq!(short.tab_title(), "○ iPhone");

        let long = Session::new(
            "d".into(),
            "Very Long Device Name Here".into(),
            "ios".into(),
            false,
        );
        assert!(long.tab_title().contains('…'));
        // Use chars().count() for character count, not byte length
        assert!(long.tab_title().chars().count() < 20);
    }

    #[test]
    fn test_session_with_config() {
        let session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        let config = LaunchConfig {
            name: "My Config".to_string(),
            ..Default::default()
        };

        let session = session.with_config(config);
        assert_eq!(session.name, "My Config");
        assert!(session.launch_config.is_some());
    }

    #[test]
    fn test_is_running() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert!(!session.is_running()); // Initializing

        session.phase = AppPhase::Running;
        assert!(session.is_running());

        session.phase = AppPhase::Reloading;
        assert!(session.is_running());

        session.phase = AppPhase::Stopped;
        assert!(!session.is_running());
    }

    #[test]
    fn test_is_busy() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);

        assert!(!session.is_busy());

        session.phase = AppPhase::Reloading;
        assert!(session.is_busy());

        session.phase = AppPhase::Running;
        assert!(!session.is_busy());
    }

    #[test]
    fn test_clear_logs() {
        let mut session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "Test");
        session.log_view_state.offset = 5;

        session.clear_logs();

        assert!(session.logs.is_empty());
        assert_eq!(session.log_view_state.offset, 0);
    }

    #[test]
    fn test_session_handle_creation() {
        let session = Session::new("d".into(), "Device".into(), "ios".into(), false);
        let handle = SessionHandle::new(session);

        assert!(!handle.has_process());
        assert!(handle.app_id().is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Filter & Search Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_session_has_filter_state() {
        use fdemon_core::{LogLevelFilter, LogSourceFilter};

        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::All);
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::All);
    }

    #[test]
    fn test_session_has_search_state() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert!(session.search_state.query.is_empty());
        assert!(!session.search_state.is_active);
    }

    #[test]
    fn test_session_cycle_level_filter() {
        use fdemon_core::LogLevelFilter;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::All);

        session.cycle_level_filter();
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::Errors);

        session.cycle_level_filter();
        assert_eq!(session.filter_state.level_filter, LogLevelFilter::Warnings);
    }

    #[test]
    fn test_session_cycle_source_filter() {
        use fdemon_core::LogSourceFilter;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::All);

        session.cycle_source_filter();
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::App);

        session.cycle_source_filter();
        assert_eq!(session.filter_state.source_filter, LogSourceFilter::Daemon);
    }

    #[test]
    fn test_session_reset_filters() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.cycle_level_filter();
        session.cycle_source_filter();
        assert!(session.has_active_filter());

        session.reset_filters();
        assert!(!session.has_active_filter());
    }

    #[test]
    fn test_session_filtered_log_indices() {
        use fdemon_core::LogLevelFilter;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "info message");
        session.log_error(LogSource::App, "error message");
        session.log_info(LogSource::Flutter, "flutter info");

        // No filter - all logs
        let indices = session.filtered_log_indices();
        assert_eq!(indices.len(), 3);
        assert_eq!(indices, vec![0, 1, 2]);

        // Errors only
        session.filter_state.level_filter = LogLevelFilter::Errors;
        let indices = session.filtered_log_indices();
        assert_eq!(indices.len(), 1);
        assert_eq!(indices[0], 1); // The error message
    }

    #[test]
    fn test_session_search_mode() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert!(!session.is_searching());

        session.start_search();
        assert!(session.is_searching());

        session.cancel_search();
        assert!(!session.is_searching());
    }

    #[test]
    fn test_session_set_search_query() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.set_search_query("error");
        assert_eq!(session.search_state.query, "error");
        assert!(session.search_state.is_valid);
    }

    #[test]
    fn test_session_clear_search() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.set_search_query("test");
        session.start_search();

        session.clear_search();

        assert!(session.search_state.query.is_empty());
        assert!(!session.search_state.is_active);
    }

    #[test]
    fn test_session_clear_logs_clears_search() {
        use fdemon_core::SearchMatch;

        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "test");
        session
            .search_state
            .update_matches(vec![SearchMatch::new(0, 0, 4)]);
        session.search_state.current_match = Some(0);

        session.clear_logs();

        assert!(session.search_state.matches.is_empty());
        assert!(session.search_state.current_match.is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Error Navigation Tests (Task 7)
    // ─────────────────────────────────────────────────────────

    fn create_session_with_logs() -> Session {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "info 0"); // index 0
        session.log_error(LogSource::App, "error 1"); // index 1
        session.log_info(LogSource::App, "info 2"); // index 2
        session.log_error(LogSource::App, "error 3"); // index 3
        session.log_info(LogSource::App, "info 4"); // index 4
        session.log_error(LogSource::App, "error 5"); // index 5
        session
    }

    #[test]
    fn test_error_indices() {
        let session = create_session_with_logs();
        let errors = session.error_indices();
        assert_eq!(errors, vec![1, 3, 5]);
    }

    #[test]
    fn test_error_count() {
        let session = create_session_with_logs();
        assert_eq!(session.error_count(), 3);
    }

    #[test]
    fn test_find_next_error_from_start() {
        let session = create_session_with_logs();
        // Scroll offset 0, should find first error at index 1
        let next = session.find_next_error();
        assert_eq!(next, Some(1));
    }

    #[test]
    fn test_find_next_error_wraps() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 5; // After last error

        let next = session.find_next_error();
        assert_eq!(next, Some(1)); // Wraps to first error
    }

    #[test]
    fn test_find_prev_error_from_end() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 5;

        let prev = session.find_prev_error();
        assert_eq!(prev, Some(3)); // Error before position 5
    }

    #[test]
    fn test_find_prev_error_wraps() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 0; // Before first error

        let prev = session.find_prev_error();
        assert_eq!(prev, Some(5)); // Wraps to last error
    }

    #[test]
    fn test_find_error_no_errors() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.log_info(LogSource::App, "info only");

        assert_eq!(session.find_next_error(), None);
        assert_eq!(session.find_prev_error(), None);
    }

    #[test]
    fn test_find_error_respects_filter() {
        use fdemon_core::LogSourceFilter;

        let mut session = create_session_with_logs();

        // Filter to App source only (all errors are from App, so all visible)
        session.filter_state.source_filter = LogSourceFilter::App;
        let errors = session.filtered_error_indices();
        assert_eq!(errors.len(), 3);

        // Filter to Daemon source (no errors)
        session.filter_state.source_filter = LogSourceFilter::Daemon;
        let errors = session.filtered_error_indices();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_find_next_error_from_middle() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 2; // Between first and second error

        let next = session.find_next_error();
        assert_eq!(next, Some(3)); // Next error after position 2
    }

    #[test]
    fn test_find_prev_error_from_middle() {
        let mut session = create_session_with_logs();
        session.log_view_state.offset = 4; // Between second and third error

        let prev = session.find_prev_error();
        assert_eq!(prev, Some(3)); // Previous error before position 4
    }

    #[test]
    fn test_error_count_empty() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert_eq!(session.error_count(), 0);
    }

    // ─────────────────────────────────────────────────────────
    // Collapse State Tests (Phase 2 Task 6)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_collapse_state_default() {
        let state = CollapseState::new();

        // With default collapsed=true, entries should show as collapsed
        assert!(!state.is_expanded(1, true));

        // With default collapsed=false, entries should show as expanded
        assert!(state.is_expanded(1, false));
    }

    #[test]
    fn test_collapse_state_toggle() {
        let mut state = CollapseState::new();

        // Toggle from collapsed (default) to expanded
        state.toggle(42, true);
        assert!(state.is_expanded(42, true));

        // Toggle back to collapsed
        state.toggle(42, true);
        assert!(!state.is_expanded(42, true));
    }

    #[test]
    fn test_collapse_state_toggle_default_expanded() {
        let mut state = CollapseState::new();

        // With default_collapsed=false, entries start expanded
        assert!(state.is_expanded(42, false));

        // Toggle to collapsed
        state.toggle(42, false);
        assert!(!state.is_expanded(42, false));

        // Toggle back to expanded
        state.toggle(42, false);
        assert!(state.is_expanded(42, false));
    }

    #[test]
    fn test_collapse_state_multiple_entries() {
        let mut state = CollapseState::new();

        state.toggle(1, true); // Expand entry 1
        state.toggle(3, true); // Expand entry 3

        assert!(state.is_expanded(1, true));
        assert!(!state.is_expanded(2, true)); // Not toggled
        assert!(state.is_expanded(3, true));
    }

    #[test]
    fn test_collapse_all() {
        let mut state = CollapseState::new();

        state.toggle(1, true);
        state.toggle(2, true);
        state.toggle(3, true);

        state.collapse_all();

        assert!(!state.is_expanded(1, true));
        assert!(!state.is_expanded(2, true));
        assert!(!state.is_expanded(3, true));
    }

    #[test]
    fn test_expand_all() {
        let mut state = CollapseState::new();

        // With default collapsed, expand all should mark entries as expanded
        state.expand_all([1, 2, 3].into_iter());

        assert!(state.is_expanded(1, true));
        assert!(state.is_expanded(2, true));
        assert!(state.is_expanded(3, true));
    }

    #[test]
    fn test_session_has_collapse_state() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        assert!(!session.collapse_state.is_expanded(1, true));
    }

    #[test]
    fn test_session_toggle_stack_trace() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Toggle stack trace for entry ID 42
        session.toggle_stack_trace(42, true);
        assert!(session.is_stack_trace_expanded(42, true));

        // Toggle again
        session.toggle_stack_trace(42, true);
        assert!(!session.is_stack_trace_expanded(42, true));
    }

    // ─────────────────────────────────────────────────────────
    // Cached Error Count Tests (Phase 2 Task 7)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_error_count_increments_on_error() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.add_log(LogEntry::info(LogSource::App, "info message"));
        assert_eq!(session.error_count(), 0);

        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        assert_eq!(session.error_count(), 1);

        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        assert_eq!(session.error_count(), 2);

        // Warnings don't count as errors
        session.add_log(LogEntry::warn(LogSource::App, "warning"));
        assert_eq!(session.error_count(), 2);
    }

    #[test]
    fn test_error_count_resets_on_clear() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        assert_eq!(session.error_count(), 2);

        session.clear_logs();
        assert_eq!(session.error_count(), 0);
    }

    #[test]
    fn test_error_count_adjusts_on_log_trim() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.max_logs = 5;

        // Add 3 errors at the start
        session.add_log(LogEntry::error(LogSource::App, "error 0"));
        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        session.add_log(LogEntry::info(LogSource::App, "info 3"));
        session.add_log(LogEntry::info(LogSource::App, "info 4"));
        assert_eq!(session.error_count(), 3);
        assert_eq!(session.logs.len(), 5);

        // Add 2 more non-error logs, which should trim the first 2 errors
        session.add_log(LogEntry::info(LogSource::App, "info 5"));
        session.add_log(LogEntry::info(LogSource::App, "info 6"));
        assert_eq!(session.logs.len(), 5);
        // First 2 errors trimmed, 1 error remains
        assert_eq!(session.error_count(), 1);
    }

    #[test]
    fn test_recalculate_error_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.add_log(LogEntry::error(LogSource::App, "error 1"));
        session.add_log(LogEntry::error(LogSource::App, "error 2"));
        session.add_log(LogEntry::info(LogSource::App, "info"));

        // Manually set wrong count (simulating a bug scenario)
        session.error_count = 999;
        assert_eq!(session.error_count(), 999);

        // Recalculate should fix it
        session.recalculate_error_count();
        assert_eq!(session.error_count(), 2);
    }

    #[test]
    fn test_error_count_with_log_helpers() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        session.log_info(LogSource::App, "info");
        assert_eq!(session.error_count(), 0);

        session.log_error(LogSource::App, "error");
        assert_eq!(session.error_count(), 1);
    }

    #[test]
    fn test_error_count_matches_actual_errors() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Add various log types
        session.add_log(LogEntry::info(LogSource::App, "info"));
        session.add_log(LogEntry::error(LogSource::Flutter, "flutter error"));
        session.add_log(LogEntry::warn(LogSource::Daemon, "warning"));
        session.add_log(LogEntry::error(
            LogSource::FlutterError,
            "flutter stderr error",
        ));
        session.add_log(LogEntry::new(LogLevel::Debug, LogSource::Watcher, "debug"));

        // Cached count should match actual count
        let actual_errors = session.logs.iter().filter(|e| e.is_error()).count();
        assert_eq!(session.error_count(), actual_errors);
        assert_eq!(session.error_count(), 2);
    }

    // ─────────────────────────────────────────────────────────
    // Logger Block Level Propagation Tests (Phase 2 Task 11)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_error_block_propagation() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Simulate Logger error block - only one line has error level
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ #0 stack trace line"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ #1 more stack trace"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // All lines should now be Error level
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Error),
            "All block lines should be Error level after propagation"
        );
    }

    #[test]
    fn test_warning_block_propagation() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Simulate Logger warning block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::warn(
            LogSource::Flutter,
            "│ ⚠ Warning: deprecated",
        ));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Additional info"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // All lines should now be Warning level
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Warning),
            "All block lines should be Warning level after propagation"
        );
    }

    #[test]
    fn test_non_block_lines_unchanged() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Regular logs (not Logger blocks)
        session.add_log(LogEntry::info(LogSource::Flutter, "Regular info"));
        session.add_log(LogEntry::error(LogSource::Flutter, "Standalone error"));
        session.add_log(LogEntry::info(LogSource::Flutter, "Another info"));

        // Levels should remain as originally set
        assert_eq!(session.logs[0].level, LogLevel::Info);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Info);
    }

    #[test]
    fn test_block_propagation_error_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Before block: 0 errors
        assert_eq!(session.error_count(), 0);

        // Add error block - only one line marked error initially
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Stack trace"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // After propagation: 4 errors (all lines promoted to Error)
        assert_eq!(session.error_count(), 4);
    }

    #[test]
    fn test_info_only_block_not_propagated() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Logger block with only Info level (e.g., debug output)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ 💡 Info: message"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Some details"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // All lines should stay Info (no propagation needed)
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Info));
        assert_eq!(session.error_count(), 0);
    }

    #[test]
    fn test_incomplete_block_not_propagated() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Block without ending (e.g., truncated output)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Stack trace"));
        // No closing └

        // Error propagation shouldn't happen (block not complete)
        assert_eq!(session.logs[0].level, LogLevel::Info); // Block start still Info
        assert_eq!(session.logs[1].level, LogLevel::Error); // Error line
        assert_eq!(session.logs[2].level, LogLevel::Info); // Stack trace still Info
    }

    #[test]
    fn test_block_end_without_start_not_propagated() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Block end without matching start (orphaned end)
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Some content"));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error: failed"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Should not propagate (scan will hit 50-line limit without finding start)
        // Actually, with only 3 lines it won't hit limit, but no ┌ means block_start == block_end
        assert_eq!(session.logs[0].level, LogLevel::Info);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Info); // The └ line stays Info
    }

    #[test]
    fn test_multiple_blocks_independent() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // First block - warning
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::warn(LogSource::Flutter, "│ ⚠ Warning"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Second block - error
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // First block should be Warning
        assert_eq!(session.logs[0].level, LogLevel::Warning);
        assert_eq!(session.logs[1].level, LogLevel::Warning);
        assert_eq!(session.logs[2].level, LogLevel::Warning);

        // Second block should be Error
        assert_eq!(session.logs[3].level, LogLevel::Error);
        assert_eq!(session.logs[4].level, LogLevel::Error);
        assert_eq!(session.logs[5].level, LogLevel::Error);
    }

    #[test]
    fn test_mixed_content_between_blocks() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Regular log
        session.add_log(LogEntry::info(LogSource::Flutter, "Regular message"));

        // Block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Another regular log
        session.add_log(LogEntry::info(LogSource::Flutter, "Another regular"));

        // Regular logs should stay Info
        assert_eq!(session.logs[0].level, LogLevel::Info);
        assert_eq!(session.logs[4].level, LogLevel::Info);

        // Block should be Error
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Error);
        assert_eq!(session.logs[3].level, LogLevel::Error);
    }

    #[test]
    fn test_block_with_leading_whitespace() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Block with leading whitespace (common in Flutter output)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "   ┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "   │ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "   └───────────────────────",
        ));

        // Should still propagate correctly
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Error));
    }

    // ─────────────────────────────────────────────────────────
    // Stateful Block Tracking Tests (Bug Fix: Logger Block Propagation)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_stateful_empty_block_handled() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Empty block (┌ immediately followed by └)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Both lines should remain Info (no errors to propagate)
        assert_eq!(session.logs.len(), 2);
        assert!(session.logs.iter().all(|e| e.level == LogLevel::Info));
    }

    #[test]
    fn test_stateful_back_to_back_blocks() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // First block (error)
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Second block (warning) - immediately after first
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::warn(LogSource::Flutter, "│ ⚠ Warning"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Third block (info only) - immediately after second
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::info(LogSource::Flutter, "│ 💡 Info"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // First block should be Error
        assert_eq!(session.logs[0].level, LogLevel::Error);
        assert_eq!(session.logs[1].level, LogLevel::Error);
        assert_eq!(session.logs[2].level, LogLevel::Error);

        // Second block should be Warning
        assert_eq!(session.logs[3].level, LogLevel::Warning);
        assert_eq!(session.logs[4].level, LogLevel::Warning);
        assert_eq!(session.logs[5].level, LogLevel::Warning);

        // Third block should remain Info (no promotion needed)
        assert_eq!(session.logs[6].level, LogLevel::Info);
        assert_eq!(session.logs[7].level, LogLevel::Info);
        assert_eq!(session.logs[8].level, LogLevel::Info);
    }

    #[test]
    fn test_stateful_block_start_trimmed_during_rotation() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);
        session.max_logs = 3;

        // Start a block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        // logs = ["┌"], block_start = Some(0)

        // Add content (within limit)
        session.add_log(LogEntry::info(LogSource::Flutter, "│ Content line 1"));
        // logs = ["┌", "│ Content 1"], block_start = Some(0)

        session.add_log(LogEntry::info(LogSource::Flutter, "│ Content line 2"));
        // logs = ["┌", "│ Content 1", "│ Content 2"], block_start = Some(0)

        // This will trigger trim, removing block start!
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        // Before trim: logs = ["┌", "│1", "│2", "│⛔"], block_start = Some(0)
        // After trim (remove 1): logs = ["│1", "│2", "│⛔"]
        // block_start was 0, which is < drain_count (1), so block_state is reset!

        // End the block - but start was trimmed, so no propagation should happen
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Block state should have been reset when block_start was trimmed
        // Only the error line should be Error level (no propagation occurred)
        let error_count = session
            .logs
            .iter()
            .filter(|e| e.level == LogLevel::Error)
            .count();
        assert_eq!(
            error_count, 1,
            "Only the explicit error line should be Error level after block_start was trimmed"
        );

        // Verify the block state was reset
        assert!(
            session.block_state.block_start.is_none(),
            "Block state should be reset"
        );
    }

    #[test]
    fn test_stateful_large_block_no_50_line_limit() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Start a block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));

        // Add 100 lines in the block (would exceed old 50-line scan limit)
        for i in 0..100 {
            session.add_log(LogEntry::info(LogSource::Flutter, format!("│ Line {}", i)));
        }

        // Add an error in the middle (but we're past the 50-line mark)
        session.add_log(LogEntry::error(
            LogSource::Flutter,
            "│ ⛔ Error at line 101",
        ));

        // More content
        for i in 0..10 {
            session.add_log(LogEntry::info(
                LogSource::Flutter,
                format!("│ Line {}", 102 + i),
            ));
        }

        // End the block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // With stateful tracking, ALL lines should be promoted to Error
        // (old implementation would fail after 50 lines)
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Error),
            "All {} lines should be Error level with stateful tracking",
            session.logs.len()
        );
    }

    #[test]
    fn test_stateful_block_state_reset_after_complete() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Complete block
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.add_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.add_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Block state should be reset
        assert!(session.block_state.block_start.is_none());
        assert_eq!(session.block_state.block_max_level, LogLevel::Info);

        // Next entry should not be affected by previous block state
        session.add_log(LogEntry::info(LogSource::Flutter, "Plain message"));
        assert_eq!(session.logs[3].level, LogLevel::Info);
    }

    // ─────────────────────────────────────────────────────────
    // Log Batching Tests (Task 04)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_log_batcher_new() {
        let batcher = LogBatcher::new();
        assert!(!batcher.has_pending());
        assert_eq!(batcher.pending_count(), 0);
        assert!(!batcher.should_flush()); // Empty batch shouldn't flush
    }

    #[test]
    fn test_log_batcher_add_single() {
        let mut batcher = LogBatcher::new();
        let entry = LogEntry::info(LogSource::App, "Test message");

        let should_flush = batcher.add(entry);

        assert!(batcher.has_pending());
        assert_eq!(batcher.pending_count(), 1);
        // Single entry shouldn't trigger flush (unless time elapsed)
        assert!(!should_flush || batcher.pending_count() >= BATCH_MAX_SIZE);
    }

    #[test]
    fn test_log_batcher_size_threshold() {
        let mut batcher = LogBatcher::new();

        // Add entries up to max size - 1
        for i in 0..(BATCH_MAX_SIZE - 1) {
            let entry = LogEntry::info(LogSource::App, format!("Log {}", i));
            let should_flush = batcher.add(entry);
            assert!(!should_flush, "Should not flush before max size");
        }

        assert_eq!(batcher.pending_count(), BATCH_MAX_SIZE - 1);

        // This entry should trigger flush due to size
        let entry = LogEntry::info(LogSource::App, "Final log");
        let should_flush = batcher.add(entry);
        assert!(should_flush, "Should flush at max size");
        assert_eq!(batcher.pending_count(), BATCH_MAX_SIZE);
    }

    #[test]
    fn test_log_batcher_flush() {
        let mut batcher = LogBatcher::new();

        batcher.add(LogEntry::info(LogSource::App, "Log 1"));
        batcher.add(LogEntry::error(LogSource::Flutter, "Log 2"));
        batcher.add(LogEntry::warn(LogSource::Daemon, "Log 3"));

        assert_eq!(batcher.pending_count(), 3);

        let entries = batcher.flush();

        assert_eq!(entries.len(), 3);
        assert!(!batcher.has_pending());
        assert_eq!(batcher.pending_count(), 0);

        // Verify entry contents
        assert_eq!(entries[0].level, LogLevel::Info);
        assert_eq!(entries[1].level, LogLevel::Error);
        assert_eq!(entries[2].level, LogLevel::Warning);
    }

    #[test]
    fn test_log_batcher_time_until_flush() {
        let batcher = LogBatcher::new();

        // Just created - should have nearly full interval remaining
        let time_remaining = batcher.time_until_flush();
        assert!(time_remaining <= BATCH_FLUSH_INTERVAL);
    }

    #[test]
    fn test_session_queue_and_flush() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue some logs
        session.queue_log(LogEntry::info(LogSource::App, "Queued 1"));
        session.queue_log(LogEntry::info(LogSource::App, "Queued 2"));
        session.queue_log(LogEntry::info(LogSource::App, "Queued 3"));

        assert!(session.has_pending_logs());
        assert_eq!(session.logs.len(), 0); // Not yet flushed to main log buffer

        // Flush the batch
        let flushed_count = session.flush_batched_logs();

        assert_eq!(flushed_count, 3);
        assert!(!session.has_pending_logs());
        assert_eq!(session.logs.len(), 3); // Now in main log buffer
    }

    #[test]
    fn test_session_add_logs_batch() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        let entries = vec![
            LogEntry::info(LogSource::App, "Batch 1"),
            LogEntry::error(LogSource::App, "Batch 2"),
            LogEntry::warn(LogSource::App, "Batch 3"),
        ];

        session.add_logs_batch(entries);

        assert_eq!(session.logs.len(), 3);
        assert_eq!(session.error_count(), 1);
    }

    #[test]
    fn test_session_batched_block_propagation() {
        // Verify block propagation works correctly with batched logs
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue a complete block
        session.queue_log(LogEntry::info(
            LogSource::Flutter,
            "┌───────────────────────",
        ));
        session.queue_log(LogEntry::error(LogSource::Flutter, "│ ⛔ Error"));
        session.queue_log(LogEntry::info(LogSource::Flutter, "│ More content"));
        session.queue_log(LogEntry::info(
            LogSource::Flutter,
            "└───────────────────────",
        ));

        // Flush the batch
        session.flush_batched_logs();

        // All lines should be promoted to Error level
        assert!(
            session.logs.iter().all(|e| e.level == LogLevel::Error),
            "Block propagation should work with batched logs"
        );
    }

    #[test]
    fn test_session_batched_error_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue logs with errors
        session.queue_log(LogEntry::info(LogSource::App, "Info"));
        session.queue_log(LogEntry::error(LogSource::App, "Error 1"));
        session.queue_log(LogEntry::warn(LogSource::App, "Warning"));
        session.queue_log(LogEntry::error(LogSource::App, "Error 2"));

        // Before flush - error count should be 0
        assert_eq!(session.error_count(), 0);

        session.flush_batched_logs();

        // After flush - error count should reflect actual errors
        assert_eq!(session.error_count(), 2);
    }

    #[test]
    fn test_log_batcher_empty_flush() {
        let mut batcher = LogBatcher::new();

        // Flush empty batcher
        let entries = batcher.flush();

        assert!(entries.is_empty());
        assert!(!batcher.has_pending());
    }

    #[test]
    fn test_session_queue_auto_flush_on_size() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        // Queue many logs - should auto-flush when we check
        for i in 0..150 {
            let should_flush =
                session.queue_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
            if should_flush {
                session.flush_batched_logs();
            }
        }

        // All logs should have been flushed to main buffer
        // (100 flushed at threshold, 50 remaining may or may not be flushed)
        assert!(session.logs.len() >= 100);
    }

    // ─────────────────────────────────────────────────────────
    // Virtualized Log Access Tests (Task 05)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_logs_range_basic() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..10 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(2, 5).collect();

        assert_eq!(range.len(), 3);
        assert!(range[0].message.contains("Log 2"));
        assert!(range[1].message.contains("Log 3"));
        assert!(range[2].message.contains("Log 4"));
    }

    #[test]
    fn test_get_logs_range_start_at_zero() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(0, 3).collect();

        assert_eq!(range.len(), 3);
        assert!(range[0].message.contains("Log 0"));
    }

    #[test]
    fn test_get_logs_range_to_end() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(3, 10).collect();

        // End is clamped to len
        assert_eq!(range.len(), 2);
        assert!(range[0].message.contains("Log 3"));
        assert!(range[1].message.contains("Log 4"));
    }

    #[test]
    fn test_get_logs_range_out_of_bounds() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(10, 20).collect();

        // Both out of bounds, should be empty
        assert!(range.is_empty());
    }

    #[test]
    fn test_get_logs_range_empty_session() {
        let session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        let range: Vec<_> = session.get_logs_range(0, 10).collect();

        assert!(range.is_empty());
    }

    #[test]
    fn test_get_logs_range_inverted_bounds() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        // Start > end (after clamping)
        let range: Vec<_> = session.get_logs_range(10, 5).collect();

        // Should handle gracefully
        assert!(range.is_empty());
    }

    #[test]
    fn test_log_count() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        assert_eq!(session.log_count(), 0);

        for i in 0..5 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        assert_eq!(session.log_count(), 5);
    }

    #[test]
    fn test_get_logs_range_full_range() {
        let mut session = Session::new("device".into(), "Device".into(), "ios".into(), false);

        for i in 0..10 {
            session.add_log(LogEntry::info(LogSource::App, format!("Log {}", i)));
        }

        let range: Vec<_> = session.get_logs_range(0, 10).collect();

        assert_eq!(range.len(), 10);
    }

    // ─────────────────────────────────────────────────────────
    // Exception Block Processing Tests (Phase 1 Task 02)
    // ─────────────────────────────────────────────────────────

    fn create_test_session() -> Session {
        Session::new(
            "device-test".to_string(),
            "Test Device".to_string(),
            "ios".to_string(),
            false,
        )
    }

    #[test]
    fn test_process_raw_line_normal() {
        let mut session = create_test_session();

        let entries = session.process_raw_line("flutter: Hello World");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Info);
        assert_eq!(entries[0].message, "Hello World"); // "flutter: " stripped
    }

    #[test]
    fn test_process_raw_line_exception_buffered() {
        let mut session = create_test_session();

        let entries =
            session.process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        assert!(entries.is_empty()); // buffered, not emitted yet
    }

    #[test]
    fn test_process_raw_line_exception_complete() {
        let mut session = create_test_session();

        // Feed exception block
        assert!(session
            .process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════")
            .is_empty());
        assert!(session.process_raw_line("Error description").is_empty());
        assert!(session
            .process_raw_line("#0      main (package:app/main.dart:15:3)")
            .is_empty());

        // Footer completes the block
        let entries =
            session.process_raw_line("════════════════════════════════════════════════════════");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
        assert!(entries[0].stack_trace.is_some());
    }

    #[test]
    fn test_process_raw_line_another_exception() {
        let mut session = create_test_session();

        let entries = session
            .process_raw_line("Another exception was thrown: RangeError (index): Invalid value");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Error);
    }

    #[test]
    fn test_flush_exception_buffer_on_exit() {
        let mut session = create_test_session();

        // Start an exception block but don't finish it
        session.process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        session.process_raw_line("Error description");

        // Flush should return partial block
        let entry = session.flush_exception_buffer();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().level, LogLevel::Error);
    }

    #[test]
    fn test_flush_exception_buffer_empty() {
        let mut session = create_test_session();

        // No pending exception
        let entry = session.flush_exception_buffer();
        assert!(entry.is_none());
    }

    #[test]
    fn test_normal_lines_after_exception() {
        let mut session = create_test_session();

        // Complete an exception block
        session.process_raw_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        session.process_raw_line("Error");
        session.process_raw_line("════════════════════════════════════════════════════════");

        // Normal lines should work after
        let entries = session.process_raw_line("Normal log message");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].level, LogLevel::Info);
    }

    #[test]
    fn test_process_raw_line_with_ansi_codes() {
        let mut session = create_test_session();

        // ANSI codes should be stripped before processing
        let entries = session.process_raw_line("\x1b[38;5;244mflutter: Test message\x1b[0m");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Test message");
        assert!(!entries[0].message.contains('\x1b')); // No ANSI codes in message
    }

    #[test]
    fn test_process_raw_line_empty_after_strip() {
        let mut session = create_test_session();

        // Empty lines should return empty vec
        let entries = session.process_raw_line("");
        assert!(entries.is_empty());

        // Whitespace-only lines
        let entries = session.process_raw_line("   ");
        assert!(entries.is_empty());
    }

    // ─────────────────────────────────────────────────────────
    // PerformanceState Tests (Phase 3, Task 05)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_performance_state_default() {
        let state = PerformanceState::default();
        assert!(!state.monitoring_active);
        assert!(state.memory_history.is_empty());
        assert!(state.gc_history.is_empty());
        assert!(state.frame_history.is_empty());
    }

    #[test]
    fn test_session_has_performance_field() {
        let session = Session::new("d".into(), "Device".into(), "android".into(), false);
        assert!(!session.performance.monitoring_active);
        assert!(session.performance.memory_history.is_empty());
    }

    #[test]
    fn test_performance_state_memory_ring_buffer_capacity() {
        use crate::session::performance::{DEFAULT_GC_HISTORY_SIZE, DEFAULT_MEMORY_HISTORY_SIZE};
        let state = PerformanceState::default();
        assert_eq!(state.memory_history.capacity(), DEFAULT_MEMORY_HISTORY_SIZE);
        assert_eq!(state.gc_history.capacity(), DEFAULT_GC_HISTORY_SIZE);
        assert_eq!(state.frame_history.capacity(), DEFAULT_FRAME_HISTORY_SIZE);
    }

    // ─────────────────────────────────────────────────────────
    // Frame Timing / Stats Computation Tests (Phase 3, Task 06)
    // ─────────────────────────────────────────────────────────

    fn make_frame(number: u64, elapsed_micros: u64) -> FrameTiming {
        FrameTiming {
            number,
            build_micros: elapsed_micros / 2,
            raster_micros: elapsed_micros / 2,
            elapsed_micros,
            timestamp: chrono::Local::now(),
        }
    }

    #[test]
    fn test_stats_computation_empty() {
        let stats = PerformanceState::compute_stats(&RingBuffer::new(10));
        assert!(stats.fps.is_none());
        assert!(stats.avg_frame_ms.is_none());
        assert_eq!(stats.jank_count, 0);
    }

    #[test]
    fn test_stats_computation_with_frames() {
        let mut frames = RingBuffer::new(100);
        for i in 0..60 {
            frames.push(make_frame(i, 10_000)); // 10ms = smooth
        }
        // Add 5 janky frames
        for i in 60..65 {
            frames.push(make_frame(i, 25_000)); // 25ms = janky
        }

        let stats = PerformanceState::compute_stats(&frames);
        assert_eq!(stats.jank_count, 5);
        assert_eq!(stats.buffered_frames, 65);
        // Average: (60*10 + 5*25) / 65 ≈ 11.15ms
        let avg = stats.avg_frame_ms.unwrap();
        assert!(avg > 11.0 && avg < 12.0);
    }

    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p95 = PerformanceState::percentile(&values, 95.0).unwrap();
        // 95th of 10 values: index = round(0.95 * 9) = round(8.55) = 9 → value 10.0
        assert!((p95 - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_percentile_empty() {
        assert!(PerformanceState::percentile(&[], 95.0).is_none());
    }

    #[test]
    fn test_percentile_single() {
        assert_eq!(PerformanceState::percentile(&[42.0], 95.0), Some(42.0));
    }

    #[test]
    fn test_jank_detection() {
        let smooth = make_frame(1, 10_000);
        let janky = make_frame(2, 20_000);
        assert!(!smooth.is_janky());
        assert!(janky.is_janky());
    }

    #[test]
    fn test_performance_state_reset_on_reconnect() {
        let mut perf = PerformanceState::default();
        perf.memory_history.push(MemoryUsage {
            heap_usage: 100,
            heap_capacity: 200,
            external_usage: 0,
            timestamp: chrono::Local::now(),
        });
        perf.monitoring_active = true;

        // Simulate reset
        perf = PerformanceState::default();
        assert!(perf.memory_history.is_empty());
        assert!(!perf.monitoring_active);
    }

    #[test]
    fn test_recompute_stats_updates_state() {
        let mut perf = PerformanceState::default();
        // Push enough frames that the totals are non-default
        for i in 0..5 {
            perf.frame_history.push(make_frame(i, 8_000)); // smooth
        }
        perf.frame_history.push(make_frame(5, 20_000)); // janky

        perf.recompute_stats();
        assert_eq!(perf.stats.buffered_frames, 6);
        assert_eq!(perf.stats.jank_count, 1);
        assert!(perf.stats.avg_frame_ms.is_some());
    }

    #[test]
    fn test_performance_stats_is_stale() {
        use fdemon_core::performance::PerformanceStats;
        let mut stats = PerformanceStats::default();
        assert!(stats.is_stale(), "default stats (no fps) should be stale");

        stats.fps = Some(60.0);
        assert!(!stats.is_stale(), "stats with fps should not be stale");
    }

    #[test]
    fn test_stats_recompute_interval_constant() {
        assert_eq!(STATS_RECOMPUTE_INTERVAL, 10);
    }

    /// Verify that `calculate_fps` returns an actual rate (frames/sec), not a
    /// raw count. We push frames with known timestamps spaced ~1/60s apart and
    /// assert the result is close to 60 FPS.
    #[test]
    fn test_calculate_fps_returns_rate_not_count() {
        let mut frames = RingBuffer::new(300);
        let base = chrono::Local::now();
        // 61 frames spanning 1 second → 60 intervals → ~60 FPS
        for i in 0..61i64 {
            let ts = base + chrono::Duration::milliseconds(i * 1000 / 60);
            frames.push(FrameTiming {
                number: i as u64,
                build_micros: 5_000,
                raster_micros: 5_000,
                elapsed_micros: 16_667,
                timestamp: ts,
            });
        }
        let fps = PerformanceState::calculate_fps(&frames);
        assert!(fps.is_some(), "should return Some fps for many frames");
        let fps_val = fps.unwrap();
        // 60 intervals / (60 * ~16.667ms / 1000) ≈ 60.0 FPS
        assert!(
            fps_val > 55.0 && fps_val < 65.0,
            "expected ~60 fps, got {fps_val}"
        );
    }

    /// Verify `compute_stats` works with only the frames parameter (no memory).
    #[test]
    fn test_compute_stats_no_memory_param() {
        let mut frames = RingBuffer::new(10);
        for i in 0..5 {
            frames.push(make_frame(i, 10_000)); // 10ms each
        }
        let stats = PerformanceState::compute_stats(&frames);
        assert_eq!(stats.buffered_frames, 5);
        assert!(stats.avg_frame_ms.is_some());
        let avg = stats.avg_frame_ms.unwrap();
        assert!((avg - 10.0).abs() < 0.001, "expected avg ~10ms, got {avg}");
    }

    /// Verify `buffered_frames` reflects the actual ring buffer size, not a
    /// lifetime count. It is capped at the buffer capacity.
    #[test]
    fn test_buffered_frames_reflects_buffer_size() {
        // 10 frames → buffered_frames == 10
        let mut frames = RingBuffer::new(300);
        for i in 0..10 {
            frames.push(make_frame(i, 8_000));
        }
        let stats = PerformanceState::compute_stats(&frames);
        assert_eq!(stats.buffered_frames, 10);

        // Push 300+ frames — buffer is capped at DEFAULT_FRAME_HISTORY_SIZE (300)
        let mut frames_full = RingBuffer::new(DEFAULT_FRAME_HISTORY_SIZE);
        for i in 0..(DEFAULT_FRAME_HISTORY_SIZE + 50) {
            frames_full.push(make_frame(i as u64, 8_000));
        }
        let stats_full = PerformanceState::compute_stats(&frames_full);
        assert_eq!(
            stats_full.buffered_frames, DEFAULT_FRAME_HISTORY_SIZE as u64,
            "buffered_frames should be capped at ring buffer capacity"
        );
    }
}
