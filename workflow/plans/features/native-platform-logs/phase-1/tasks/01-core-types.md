## Task: Extend Core Types for Native Log Sources

**Objective**: Add `LogSource::Native` variant, `NativeLogPriority` enum, and update `LogSourceFilter` to support filtering native platform logs. This is the foundational type work that all other tasks depend on.

**Depends on**: None

### Scope

- `crates/fdemon-core/src/types.rs`: Extend `LogSource`, `LogSourceFilter`, `FilterState`, add `NativeLogPriority`

### Details

#### 1. Add `LogSource::Native { tag: String }` variant

Current `LogSource` enum (types.rs:229–256) has 6 variants: `App`, `Daemon`, `Flutter`, `FlutterError`, `Watcher`, `VmService`.

Add a new variant:

```rust
pub enum LogSource {
    App,
    Daemon,
    Flutter,
    FlutterError,
    Watcher,
    VmService,
    Native { tag: String },  // NEW: native platform log with arbitrary tag name
}
```

Update `LogSource::prefix()` (types.rs:246–256):
```rust
LogSource::Native { ref tag } => tag.as_str(),
```

The `prefix()` method returns `&str`, but the `Native` variant carries an owned `String`. Return `tag.as_str()` — the borrow is tied to `&self` so this works.

**Important**: `LogSource` derives `Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize`. The `Native` variant with `String` field is compatible with all these derives. However, check that any `match` statements on `LogSource` throughout the codebase are updated (the compiler will enforce this via exhaustive matching).

#### 2. Add `NativeLogPriority` enum

Add a new enum representing Android logcat / native log priority levels, with mapping to `LogLevel`:

```rust
/// Priority levels from native platform logging (Android logcat, macOS unified logging).
/// Maps to the project's `LogLevel` for filtering and display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NativeLogPriority {
    Verbose,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

impl NativeLogPriority {
    /// Map native priority to fdemon's LogLevel.
    pub fn to_log_level(self) -> LogLevel {
        match self {
            Self::Verbose | Self::Debug => LogLevel::Debug,
            Self::Info => LogLevel::Info,
            Self::Warning => LogLevel::Warning,
            Self::Error | Self::Fatal => LogLevel::Error,
        }
    }

    /// Parse from Android logcat single-character priority.
    pub fn from_logcat_char(c: char) -> Option<Self> {
        match c {
            'V' => Some(Self::Verbose),
            'D' => Some(Self::Debug),
            'I' => Some(Self::Info),
            'W' => Some(Self::Warning),
            'E' => Some(Self::Error),
            'F' => Some(Self::Fatal),
            _ => None,
        }
    }

    /// Parse from macOS unified logging level string.
    pub fn from_macos_level(level: &str) -> Option<Self> {
        match level.to_lowercase().as_str() {
            "default" | "notice" => Some(Self::Info),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "error" => Some(Self::Error),
            "fault" => Some(Self::Fatal),
            _ => None,
        }
    }
}
```

#### 3. Update `LogSourceFilter` — add `Native` variant in cycle

Current cycle (types.rs:259–312): `All → App → Daemon → Flutter → Watcher → All`

New cycle: `All → App → Daemon → Flutter → Native → Watcher → All`

```rust
pub enum LogSourceFilter {
    All,
    App,
    Daemon,
    Flutter,
    Native,   // NEW
    Watcher,
}

impl LogSourceFilter {
    pub fn cycle(&self) -> Self {
        match self {
            Self::All => Self::App,
            Self::App => Self::Daemon,
            Self::Daemon => Self::Flutter,
            Self::Flutter => Self::Native,    // NEW
            Self::Native => Self::Watcher,    // NEW
            Self::Watcher => Self::All,
        }
    }

    pub fn matches(&self, source: &LogSource) -> bool {
        match self {
            Self::All => true,
            Self::App => matches!(source, LogSource::App),
            Self::Daemon => matches!(source, LogSource::Daemon),
            Self::Flutter => matches!(source, LogSource::Flutter | LogSource::FlutterError | LogSource::VmService),
            Self::Native => matches!(source, LogSource::Native { .. }),   // NEW
            Self::Watcher => matches!(source, LogSource::Watcher),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::All => "All sources",
            Self::App => "App logs",
            Self::Daemon => "Daemon logs",
            Self::Flutter => "Flutter logs",
            Self::Native => "Native logs",   // NEW
            Self::Watcher => "Watcher logs",
        }
    }
}
```

#### 4. Fix all exhaustive match sites

The Rust compiler will flag every `match` on `LogSource` and `LogSourceFilter` that's missing the new variants. Search the workspace for all match sites and add the new arms. Known locations from research:

- `types.rs`: `LogSource::prefix()`, `LogSourceFilter::cycle()`, `matches()`, `display_name()`
- `fdemon-tui/src/widgets/log_view/mod.rs`: `source_style()` (line 207) — defer to task 08, but add a catch-all arm here to compile: `LogSource::Native { .. } => Style::default().fg(palette::ACCENT)`
- Any `Serialize`/`Deserialize` usage of `LogSource` in tests or snapshots

### Acceptance Criteria

1. `LogSource::Native { tag: "GoLog".into() }` can be constructed and matched
2. `NativeLogPriority::from_logcat_char('E')` returns `Some(NativeLogPriority::Error)`
3. `NativeLogPriority::Error.to_log_level()` returns `LogLevel::Error`
4. `NativeLogPriority::from_macos_level("fault")` returns `Some(NativeLogPriority::Fatal)`
5. `LogSourceFilter::Flutter.cycle()` returns `LogSourceFilter::Native`
6. `LogSourceFilter::Native.cycle()` returns `LogSourceFilter::Watcher`
7. `LogSourceFilter::Native.matches(&LogSource::Native { tag: "GoLog".into() })` returns `true`
8. `LogSourceFilter::Native.matches(&LogSource::App)` returns `false`
9. `FilterState::matches()` correctly passes/rejects `Native` entries based on source filter
10. `LogSource::Native { tag: "GoLog".into() }.prefix()` returns `"GoLog"`
11. Workspace compiles: `cargo check --workspace`
12. All existing tests pass: `cargo test --workspace`

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_log_source_prefix() {
        let source = LogSource::Native { tag: "GoLog".into() };
        assert_eq!(source.prefix(), "GoLog");
    }

    #[test]
    fn test_native_log_source_filter_cycle() {
        assert_eq!(LogSourceFilter::Flutter.cycle(), LogSourceFilter::Native);
        assert_eq!(LogSourceFilter::Native.cycle(), LogSourceFilter::Watcher);
    }

    #[test]
    fn test_native_log_source_filter_matches() {
        let native = LogSource::Native { tag: "GoLog".into() };
        assert!(LogSourceFilter::All.matches(&native));
        assert!(LogSourceFilter::Native.matches(&native));
        assert!(!LogSourceFilter::App.matches(&native));
        assert!(!LogSourceFilter::Flutter.matches(&native));
    }

    #[test]
    fn test_native_log_priority_from_logcat_char() {
        assert_eq!(NativeLogPriority::from_logcat_char('V'), Some(NativeLogPriority::Verbose));
        assert_eq!(NativeLogPriority::from_logcat_char('D'), Some(NativeLogPriority::Debug));
        assert_eq!(NativeLogPriority::from_logcat_char('I'), Some(NativeLogPriority::Info));
        assert_eq!(NativeLogPriority::from_logcat_char('W'), Some(NativeLogPriority::Warning));
        assert_eq!(NativeLogPriority::from_logcat_char('E'), Some(NativeLogPriority::Error));
        assert_eq!(NativeLogPriority::from_logcat_char('F'), Some(NativeLogPriority::Fatal));
        assert_eq!(NativeLogPriority::from_logcat_char('X'), None);
    }

    #[test]
    fn test_native_log_priority_to_log_level() {
        assert_eq!(NativeLogPriority::Verbose.to_log_level(), LogLevel::Debug);
        assert_eq!(NativeLogPriority::Debug.to_log_level(), LogLevel::Debug);
        assert_eq!(NativeLogPriority::Info.to_log_level(), LogLevel::Info);
        assert_eq!(NativeLogPriority::Warning.to_log_level(), LogLevel::Warning);
        assert_eq!(NativeLogPriority::Error.to_log_level(), LogLevel::Error);
        assert_eq!(NativeLogPriority::Fatal.to_log_level(), LogLevel::Error);
    }

    #[test]
    fn test_native_log_priority_from_macos_level() {
        assert_eq!(NativeLogPriority::from_macos_level("debug"), Some(NativeLogPriority::Debug));
        assert_eq!(NativeLogPriority::from_macos_level("info"), Some(NativeLogPriority::Info));
        assert_eq!(NativeLogPriority::from_macos_level("default"), Some(NativeLogPriority::Info));
        assert_eq!(NativeLogPriority::from_macos_level("notice"), Some(NativeLogPriority::Info));
        assert_eq!(NativeLogPriority::from_macos_level("error"), Some(NativeLogPriority::Error));
        assert_eq!(NativeLogPriority::from_macos_level("fault"), Some(NativeLogPriority::Fatal));
        assert_eq!(NativeLogPriority::from_macos_level("unknown"), None);
    }

    #[test]
    fn test_filter_state_matches_native() {
        let native_entry = LogEntry::new(
            LogLevel::Info,
            LogSource::Native { tag: "GoLog".into() },
            "test message".to_string(),
        );

        let mut filter = FilterState::default();
        assert!(filter.matches(&native_entry)); // All/All passes everything

        filter.source_filter = LogSourceFilter::Native;
        assert!(filter.matches(&native_entry));

        filter.source_filter = LogSourceFilter::App;
        assert!(!filter.matches(&native_entry));
    }
}
```

### Notes

- `LogSource::Native { tag }` uses an owned `String` because native tags are arbitrary (not a fixed set). The `Eq`/`Hash`/`Clone` derives still work.
- `NativeLogPriority` is separate from `LogLevel` because it captures platform-specific granularity (Verbose, Fatal) that `LogLevel` doesn't have. The `to_log_level()` mapping is lossy by design.
- The `from_macos_level` method maps both `"default"` and `"notice"` to `Info` — these are macOS unified logging levels that don't have direct equivalents in Android's scheme.
- When adding the `Native` match arm to `source_style()` in the TUI crate, use a temporary placeholder color to unblock compilation. Task 08 will set the final color.

---

## Completion Summary

**Status:** Not Started
