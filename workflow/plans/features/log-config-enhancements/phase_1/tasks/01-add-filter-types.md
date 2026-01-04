## Task: Add Log Filter Types

**Objective**: Define the core filter type enums and filter state struct that will be used throughout the log filtering system.

**Depends on**: None

**Estimated Time**: 3-4 hours

### Scope

- `src/core/types.rs`: Add filter type definitions

### Details

Add the following types to `src/core/types.rs`:

1. **LogLevelFilter** enum:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
   pub enum LogLevelFilter {
       #[default]
       All,
       Errors,
       Warnings,
       Info,
       Debug,
   }
   ```

2. **LogSourceFilter** enum:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
   pub enum LogSourceFilter {
       #[default]
       All,
       App,
       Daemon,
       Flutter,
       Watcher,
   }
   ```

3. **FilterState** struct:
   ```rust
   #[derive(Debug, Clone, Default)]
   pub struct FilterState {
       pub level_filter: LogLevelFilter,
       pub source_filter: LogSourceFilter,
   }
   ```

4. Implement helper methods:
   - `LogLevelFilter::cycle()` - cycle to next filter option
   - `LogLevelFilter::matches(&LogLevel) -> bool` - check if log level passes filter
   - `LogLevelFilter::display_name() -> &'static str` - for UI display
   - `LogSourceFilter::cycle()` - cycle to next filter option
   - `LogSourceFilter::matches(&LogSource) -> bool` - check if log source passes filter
   - `LogSourceFilter::display_name() -> &'static str` - for UI display
   - `FilterState::reset()` - reset both filters to All
   - `FilterState::is_active() -> bool` - true if any filter is not All
   - `FilterState::matches(&LogEntry) -> bool` - check if entry passes both filters

### Acceptance Criteria

1. All types are defined and exported from `core/types.rs`
2. `LogLevelFilter::matches()` correctly filters:
   - `All` matches all levels
   - `Errors` matches only `LogLevel::Error`
   - `Warnings` matches `Warning` and `Error`
   - `Info` matches `Info`, `Warning`, and `Error`
   - `Debug` matches all levels (same as All)
3. `LogSourceFilter::matches()` correctly filters:
   - `All` matches all sources
   - `App` matches only `LogSource::App`
   - `Daemon` matches only `LogSource::Daemon`
   - `Flutter` matches `LogSource::Flutter` and `LogSource::FlutterError`
   - `Watcher` matches only `LogSource::Watcher`
4. `cycle()` methods wrap around correctly
5. `display_name()` returns user-friendly strings ("Errors only", "App logs", etc.)
6. Types derive necessary traits for use in state (Debug, Clone, Default, PartialEq)

### Testing

Add unit tests to `src/core/types.rs`:

```rust
#[cfg(test)]
mod filter_tests {
    use super::*;

    #[test]
    fn test_level_filter_cycle() {
        let mut f = LogLevelFilter::All;
        f = f.cycle();
        assert_eq!(f, LogLevelFilter::Errors);
        // ... continue through all variants and wrap
    }

    #[test]
    fn test_level_filter_matches_errors_only() {
        let filter = LogLevelFilter::Errors;
        assert!(filter.matches(&LogLevel::Error));
        assert!(!filter.matches(&LogLevel::Warning));
        assert!(!filter.matches(&LogLevel::Info));
    }

    #[test]
    fn test_source_filter_matches_flutter() {
        let filter = LogSourceFilter::Flutter;
        assert!(filter.matches(&LogSource::Flutter));
        assert!(filter.matches(&LogSource::FlutterError));
        assert!(!filter.matches(&LogSource::App));
    }

    #[test]
    fn test_filter_state_matches_combined() {
        let state = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::Flutter,
        };
        let entry = LogEntry::error(LogSource::Flutter, "test");
        assert!(state.matches(&entry));

        let entry2 = LogEntry::info(LogSource::Flutter, "test");
        assert!(!state.matches(&entry2)); // wrong level

        let entry3 = LogEntry::error(LogSource::App, "test");
        assert!(!state.matches(&entry3)); // wrong source
    }

    #[test]
    fn test_filter_state_is_active() {
        let default = FilterState::default();
        assert!(!default.is_active());

        let with_level = FilterState {
            level_filter: LogLevelFilter::Errors,
            ..Default::default()
        };
        assert!(with_level.is_active());
    }
}
```

### Notes

- The `Warnings` filter should show warnings AND errors (cumulative), similar to log level filtering in most logging frameworks
- Consider if we want a separate "Warnings Only" option in the future
- `FlutterError` source should be grouped with `Flutter` for source filtering purposes