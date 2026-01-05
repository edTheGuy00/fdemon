## Task: Terminal Hyperlink Capability Detection

**Objective**: Implement detection of terminal hyperlink (OSC 8) support by checking environment variables and applying heuristics for known terminal emulators.

**Depends on**: [01-hyperlink-module-url-generation](01-hyperlink-module-url-generation.md)

### Scope

- `src/tui/hyperlinks.rs`: Add detection functions
- `src/app/state.rs`: Cache detection result at startup

### Background

Not all terminals support OSC 8 hyperlinks. Before emitting hyperlink escape sequences, we need to detect whether the terminal supports them. Unsupported terminals may:
- Display garbage characters
- Ignore the sequences (harmless but useless)
- Behave unpredictably

### Terminals with OSC 8 Support

| Terminal | Status | Detection Method |
|----------|--------|------------------|
| iTerm2 | ✅ Full | `$TERM_PROGRAM = "iTerm.app"` |
| Kitty | ✅ Full | `$TERM = "xterm-kitty"` |
| WezTerm | ✅ Full | `$TERM_PROGRAM = "WezTerm"` |
| Alacritty | ✅ Full (0.11+) | `$TERM = "alacritty"` |
| Windows Terminal | ✅ Full | `$WT_SESSION` exists |
| GNOME Terminal | ✅ 3.26+ | `$VTE_VERSION >= 5000` |
| Konsole | ✅ 18.07+ | `$KONSOLE_VERSION >= 180700` |
| foot | ✅ Full | `$TERM = "foot"` |
| mintty | ✅ Full | `$TERM_PROGRAM = "mintty"` |
| Hyper | ✅ Full | `$TERM_PROGRAM = "Hyper"` |
| VS Code Terminal | ✅ Full | `$TERM_PROGRAM = "vscode"` |
| macOS Terminal.app | ❌ No | `$TERM_PROGRAM = "Apple_Terminal"` |
| tmux | ⚠️ Passthrough | `$TERM` starts with `"tmux"` |
| screen | ❌ No | `$TERM` starts with `"screen"` |

### Implementation Details

#### 3. Detection Function (Terminal Only)

> **Note**: IDE detection is handled by `detect_parent_ide()` in Task 02. This function focuses on terminal type detection.

```rust
// In src/tui/hyperlinks.rs

use std::env;

/// Result of hyperlink support detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyperlinkSupport {
    /// Terminal definitely supports OSC 8
    Supported,
    /// Terminal definitely does not support OSC 8
    Unsupported,
    /// Unknown - terminal may or may not support OSC 8
    Unknown,
}

impl HyperlinkSupport {
    /// Convert to boolean, treating Unknown as unsupported (conservative)
    pub fn is_supported_conservative(&self) -> bool {
        matches!(self, HyperlinkSupport::Supported)
    }
    
    /// Convert to boolean, treating Unknown as supported (optimistic)
    pub fn is_supported_optimistic(&self) -> bool {
        !matches!(self, HyperlinkSupport::Unsupported)
    }
}

/// Detect terminal hyperlink support
/// 
/// Checks environment variables and applies heuristics to determine
/// whether the current terminal supports OSC 8 hyperlinks.
pub fn detect_hyperlink_support() -> HyperlinkSupport {
    // Check for explicitly unsupported terminals first
    if is_unsupported_terminal() {
        return HyperlinkSupport::Unsupported;
    }
    
    // Check for known supported terminals
    if is_supported_terminal() {
        return HyperlinkSupport::Supported;
    }
    
    // Check for terminal multiplexers (passthrough may work)
    if is_terminal_multiplexer() {
        return HyperlinkSupport::Unknown;
    }
    
    // Default to unknown
    HyperlinkSupport::Unknown
}

/// Check for terminals known to NOT support OSC 8
fn is_unsupported_terminal() -> bool {
    // macOS Terminal.app
    if env::var("TERM_PROGRAM").as_deref() == Ok("Apple_Terminal") {
        return true;
    }
    
    // screen (not tmux)
    if let Ok(term) = env::var("TERM") {
        if term.starts_with("screen") && !term.contains("tmux") {
            return true;
        }
    }
    
    // Dumb terminal
    if env::var("TERM").as_deref() == Ok("dumb") {
        return true;
    }
    
    false
}

/// Check for terminals known to support OSC 8
fn is_supported_terminal() -> bool {
    // iTerm2
    if env::var("TERM_PROGRAM").as_deref() == Ok("iTerm.app") {
        return true;
    }
    
    // WezTerm
    if env::var("TERM_PROGRAM").as_deref() == Ok("WezTerm") {
        return true;
    }
    
    // VS Code integrated terminal
    if env::var("TERM_PROGRAM").as_deref() == Ok("vscode") {
        return true;
    }
    
    // Hyper
    if env::var("TERM_PROGRAM").as_deref() == Ok("Hyper") {
        return true;
    }
    
    // mintty (Git Bash, etc.)
    if env::var("TERM_PROGRAM").as_deref() == Ok("mintty") {
        return true;
    }
    
    // Windows Terminal
    if env::var("WT_SESSION").is_ok() {
        return true;
    }
    
    // Check TERM variable
    if let Ok(term) = env::var("TERM") {
        // Kitty
        if term == "xterm-kitty" {
            return true;
        }
        
        // Alacritty
        if term == "alacritty" {
            return true;
        }
        
        // foot
        if term == "foot" || term == "foot-extra" {
            return true;
        }
    }
    
    // GNOME Terminal (VTE-based) 3.26+
    if let Ok(vte_version) = env::var("VTE_VERSION") {
        if let Ok(version) = vte_version.parse::<u32>() {
            // VTE 0.50.0+ supports OSC 8 (version number is 5000+)
            if version >= 5000 {
                return true;
            }
        }
    }
    
    // Konsole 18.07+
    if let Ok(konsole_version) = env::var("KONSOLE_VERSION") {
        if let Ok(version) = konsole_version.parse::<u32>() {
            if version >= 180700 {
                return true;
            }
        }
    }
    
    false
}

/// Check if running inside a terminal multiplexer
fn is_terminal_multiplexer() -> bool {
    // tmux
    if env::var("TMUX").is_ok() {
        return true;
    }
    
    // Check TERM for tmux
    if let Ok(term) = env::var("TERM") {
        if term.starts_with("tmux") || term.starts_with("screen") {
            return true;
        }
    }
    
    false
}
```

#### 2. Cached Detection

```rust
// In src/tui/hyperlinks.rs

use std::sync::OnceLock;

/// Cached hyperlink support detection result
static HYPERLINK_SUPPORT: OnceLock<HyperlinkSupport> = OnceLock::new();

/// Get cached hyperlink support detection result
/// Detection is performed once on first call and cached
pub fn hyperlink_support() -> HyperlinkSupport {
    *HYPERLINK_SUPPORT.get_or_init(detect_hyperlink_support)
}

/// Force re-detection of hyperlink support (mainly for testing)
#[cfg(test)]
pub fn reset_hyperlink_detection() {
    // OnceLock doesn't have a reset, so we'd need a different approach for testing
    // In tests, call detect_hyperlink_support() directly
}
```

#### 3. Integration with HyperlinkMode

```rust
// In src/tui/hyperlinks.rs

impl HyperlinkMode {
    /// Determine if hyperlinks should be enabled based on mode and detection
    pub fn should_enable(&self) -> bool {
        match self {
            HyperlinkMode::Enabled => true,
            HyperlinkMode::Disabled => false,
            HyperlinkMode::Auto => {
                hyperlink_support().is_supported_conservative()
            }
        }
    }
    
    /// Get a human-readable status of hyperlink support
    pub fn status_message(&self) -> String {
        match self {
            HyperlinkMode::Enabled => "Hyperlinks: Enabled (forced)".to_string(),
            HyperlinkMode::Disabled => "Hyperlinks: Disabled".to_string(),
            HyperlinkMode::Auto => {
                match hyperlink_support() {
                    HyperlinkSupport::Supported => "Hyperlinks: Auto (supported)".to_string(),
                    HyperlinkSupport::Unsupported => "Hyperlinks: Auto (unsupported)".to_string(),
                    HyperlinkSupport::Unknown => "Hyperlinks: Auto (unknown terminal)".to_string(),
                }
            }
        }
    }
}
```

#### 4. Terminal Info for Debugging

```rust
/// Get information about the detected terminal for debugging
pub fn terminal_info() -> TerminalInfo {
    TerminalInfo {
        term: env::var("TERM").ok(),
        term_program: env::var("TERM_PROGRAM").ok(),
        colorterm: env::var("COLORTERM").ok(),
        wt_session: env::var("WT_SESSION").is_ok(),
        tmux: env::var("TMUX").is_ok(),
        vte_version: env::var("VTE_VERSION").ok(),
        konsole_version: env::var("KONSOLE_VERSION").ok(),
        hyperlink_support: detect_hyperlink_support(),
    }
}

#[derive(Debug, Clone)]
pub struct TerminalInfo {
    pub term: Option<String>,
    pub term_program: Option<String>,
    pub colorterm: Option<String>,
    pub wt_session: bool,
    pub tmux: bool,
    pub vte_version: Option<String>,
    pub konsole_version: Option<String>,
    pub hyperlink_support: HyperlinkSupport,
}

impl std::fmt::Display for TerminalInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Terminal Information:")?;
        writeln!(f, "  TERM: {:?}", self.term)?;
        writeln!(f, "  TERM_PROGRAM: {:?}", self.term_program)?;
        writeln!(f, "  COLORTERM: {:?}", self.colorterm)?;
        writeln!(f, "  WT_SESSION: {}", self.wt_session)?;
        writeln!(f, "  TMUX: {}", self.tmux)?;
        writeln!(f, "  VTE_VERSION: {:?}", self.vte_version)?;
        writeln!(f, "  KONSOLE_VERSION: {:?}", self.konsole_version)?;
        writeln!(f, "  Hyperlink Support: {:?}", self.hyperlink_support)?;
        Ok(())
    }
}
```

### Acceptance Criteria

1. [ ] `HyperlinkSupport` enum with Supported, Unsupported, Unknown variants
2. [ ] `detect_hyperlink_support()` correctly identifies known terminals
3. [ ] iTerm2, Kitty, WezTerm, Alacritty detected as supported
4. [ ] macOS Terminal.app detected as unsupported
5. [ ] Windows Terminal detected via WT_SESSION
6. [ ] VTE-based terminals (GNOME Terminal) detected via VTE_VERSION
7. [ ] Terminal multiplexers (tmux, screen) detected
8. [ ] Detection result cached with OnceLock
9. [ ] `HyperlinkMode::should_enable()` respects both mode and detection
10. [ ] `terminal_info()` provides debugging information
11. [ ] All detection logic has unit tests

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ParentIde;
    use std::env;

    // Note: These tests modify environment variables and should be run serially
    // Use `cargo test -- --test-threads=1` for reliable results

    fn with_env<F, R>(key: &str, value: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let original = env::var(key).ok();
        env::set_var(key, value);
        let result = f();
        match original {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
        result
    }

    fn without_env<F, R>(key: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let original = env::var(key).ok();
        env::remove_var(key);
        let result = f();
        if let Some(v) = original {
            env::set_var(key, v);
        }
        result
    }

    #[test]
    fn test_iterm2_supported() {
        with_env("TERM_PROGRAM", "iTerm.app", || {
            assert!(is_supported_terminal());
        });
    }

    #[test]
    fn test_wezterm_supported() {
        with_env("TERM_PROGRAM", "WezTerm", || {
            assert!(is_supported_terminal());
        });
    }

    #[test]
    fn test_kitty_supported() {
        with_env("TERM", "xterm-kitty", || {
            without_env("TERM_PROGRAM", || {
                assert!(is_supported_terminal());
            });
        });
    }

    #[test]
    fn test_alacritty_supported() {
        with_env("TERM", "alacritty", || {
            without_env("TERM_PROGRAM", || {
                assert!(is_supported_terminal());
            });
        });
    }

    #[test]
    fn test_windows_terminal_supported() {
        with_env("WT_SESSION", "some-uuid", || {
            assert!(is_supported_terminal());
        });
    }

    #[test]
    fn test_vte_new_version_supported() {
        with_env("VTE_VERSION", "6003", || {
            without_env("TERM_PROGRAM", || {
                assert!(is_supported_terminal());
            });
        });
    }

    #[test]
    fn test_vte_old_version_unsupported() {
        with_env("VTE_VERSION", "4500", || {
            without_env("TERM_PROGRAM", || {
                assert!(!is_supported_terminal());
            });
        });
    }

    #[test]
    fn test_apple_terminal_unsupported() {
        with_env("TERM_PROGRAM", "Apple_Terminal", || {
            assert!(is_unsupported_terminal());
        });
    }

    #[test]
    fn test_screen_unsupported() {
        with_env("TERM", "screen", || {
            without_env("TERM_PROGRAM", || {
                assert!(is_unsupported_terminal());
            });
        });
    }

    #[test]
    fn test_dumb_terminal_unsupported() {
        with_env("TERM", "dumb", || {
            assert!(is_unsupported_terminal());
        });
    }

    #[test]
    fn test_tmux_is_multiplexer() {
        with_env("TMUX", "/tmp/tmux-1000/default,12345,0", || {
            assert!(is_terminal_multiplexer());
        });
    }

    #[test]
    fn test_hyperlink_support_conservative() {
        assert!(HyperlinkSupport::Supported.is_supported_conservative());
        assert!(!HyperlinkSupport::Unsupported.is_supported_conservative());
        assert!(!HyperlinkSupport::Unknown.is_supported_conservative());
    }

    #[test]
    fn test_hyperlink_support_optimistic() {
        assert!(HyperlinkSupport::Supported.is_supported_optimistic());
        assert!(!HyperlinkSupport::Unsupported.is_supported_optimistic());
        assert!(HyperlinkSupport::Unknown.is_supported_optimistic());
    }

    #[test]
    fn test_hyperlink_mode_enabled_always_true() {
        // This test would need mocking of hyperlink_support()
        // or direct testing of the logic
        assert!(HyperlinkMode::Enabled.is_enabled(false));
        assert!(HyperlinkMode::Enabled.is_enabled(true));
    }

    #[test]
    fn test_hyperlink_mode_disabled_always_false() {
        assert!(!HyperlinkMode::Disabled.is_enabled(false));
        assert!(!HyperlinkMode::Disabled.is_enabled(true));
    }

    #[test]
    fn test_terminal_info_display() {
        let info = terminal_info();
        let display = format!("{}", info);
        assert!(display.contains("Terminal Information:"));
        assert!(display.contains("TERM:"));
        assert!(display.contains("Hyperlink Support:"));
    }
}
```

### Run Tests

```bash
# Run hyperlink tests (single-threaded for env var safety)
cargo test tui::hyperlinks -- --test-threads=1

# Verify detection on current terminal
cargo run -- --debug-terminal-info
```

### Notes

- Environment variable checks are fast and don't require process spawning
- The OnceLock cache ensures detection only happens once per process
- For tmux, hyperlinks may work with `allow-passthrough` option
- Some terminals may support OSC 8 but not be in our detection list - users can force enable
- Consider adding a `--debug-terminal-info` CLI flag for troubleshooting

### Edge Cases

1. **Nested terminals**: tmux inside iTerm2 - the inner terminal env vars may mask outer
2. **SSH sessions**: Terminal detection may not work correctly over SSH
3. **Docker containers**: Minimal environments may lack env vars
4. **Custom TERM values**: Users with custom terminfo entries

### References

- [OSC 8 Hyperlink Specification](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
- [Terminal Feature Detection](https://invisible-island.net/xterm/xterm.faq.html)

### Estimated Time

2-3 hours

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/hyperlinks.rs` | Add detection functions, caching, TerminalInfo |