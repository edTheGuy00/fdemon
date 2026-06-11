//! Clipboard backend detection.
//!
//! Inspects the runtime environment (SSH session, terminal multiplexer,
//! display server, TTY) and maps the configured [`ClipboardMode`] to a
//! concrete backend choice. The decision logic is pure ([`choose_backend`])
//! so it can be unit-tested with synthetic environments; only
//! [`ClipboardEnv::from_process_env`] touches process state.

use std::io::IsTerminal;

use crate::config::ClipboardMode;

use super::osc52::Osc52Mode;

/// Snapshot of the environment facts that drive clipboard backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardEnv {
    /// Running inside an SSH session (`SSH_TTY`, `SSH_CONNECTION`, or
    /// `SSH_CLIENT` is set and non-empty).
    pub ssh: bool,
    /// Running inside GNU screen (`STY` set, or `TERM` starts with "screen"
    /// while *not* inside tmux — tmux also sets `TERM=screen-256color` but
    /// must receive raw OSC 52, not screen's DCS wrapping).
    pub screen: bool,
    /// A display server is reachable: `DISPLAY`/`WAYLAND_DISPLAY` on Linux;
    /// always `true` on macOS and Windows where the OS clipboard needs no
    /// display server.
    pub display: bool,
    /// stdout is attached to a terminal, so OSC 52 sequences can reach the
    /// user's terminal emulator.
    pub stdout_is_tty: bool,
}

impl ClipboardEnv {
    /// Capture the clipboard-relevant facts from the real process environment.
    pub fn from_process_env() -> Self {
        Self::from_env_lookup(
            |name| std::env::var(name).ok(),
            std::io::stdout().is_terminal(),
        )
    }

    /// Build the snapshot from an injectable env lookup (tests use a map
    /// instead of mutating process env, which races with parallel tests).
    fn from_env_lookup(get: impl Fn(&str) -> Option<String>, stdout_is_tty: bool) -> Self {
        let set = |name: &str| get(name).map(|v| !v.is_empty()).unwrap_or(false);
        let term_is_screen = get("TERM")
            .map(|t| t.starts_with("screen"))
            .unwrap_or(false);
        Self {
            ssh: set("SSH_TTY") || set("SSH_CONNECTION") || set("SSH_CLIENT"),
            // tmux sets TERM=screen-256color by default but forwards raw
            // OSC 52 itself; only treat this as GNU screen when not in tmux.
            screen: (set("STY") || term_is_screen) && !set("TMUX"),
            display: cfg!(any(target_os = "macos", target_os = "windows"))
                || set("DISPLAY")
                || set("WAYLAND_DISPLAY"),
            stdout_is_tty,
        }
    }

    /// OSC 52 emission mode for this environment: GNU screen needs the
    /// sequence wrapped in chunked DCS, everything else (including tmux,
    /// which forwards raw OSC 52 when `set-clipboard on`) takes it plain.
    pub fn osc52_mode(&self) -> Osc52Mode {
        if self.screen {
            Osc52Mode::Screen
        } else {
            Osc52Mode::Plain
        }
    }
}

/// Concrete backend choice produced by [`choose_backend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendChoice {
    /// Try the OS clipboard first; if it cannot be initialised (no display
    /// server), fall back to OSC 52 when `Some(mode)` is given, otherwise
    /// disable copy.
    System { osc52_fallback: Option<Osc52Mode> },
    /// Emit OSC 52 escape sequences to the terminal.
    Osc52(Osc52Mode),
    /// Copy is disabled; every write reports an error.
    Disabled,
}

/// Map the configured [`ClipboardMode`] and detected [`ClipboardEnv`] to a
/// backend choice.
///
/// In `Auto` mode an SSH session always prefers OSC 52 — even with X11
/// forwarding (`DISPLAY` set), the OS clipboard would land on the *remote*
/// machine while the user's clipboard lives on their local machine, which
/// only OSC 52 can reach through the terminal.
pub fn choose_backend(mode: ClipboardMode, env: &ClipboardEnv) -> BackendChoice {
    match mode {
        ClipboardMode::Off => BackendChoice::Disabled,
        ClipboardMode::System => BackendChoice::System {
            osc52_fallback: None,
        },
        ClipboardMode::Osc52 => BackendChoice::Osc52(env.osc52_mode()),
        ClipboardMode::Auto => {
            if env.ssh && env.stdout_is_tty {
                BackendChoice::Osc52(env.osc52_mode())
            } else if env.display {
                BackendChoice::System {
                    osc52_fallback: env.stdout_is_tty.then(|| env.osc52_mode()),
                }
            } else if env.stdout_is_tty {
                // No display server (headless box, local console): the OS
                // clipboard cannot work, go straight to OSC 52.
                BackendChoice::Osc52(env.osc52_mode())
            } else {
                BackendChoice::Disabled
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(ssh: bool, screen: bool, display: bool, tty: bool) -> ClipboardEnv {
        ClipboardEnv {
            ssh,
            screen,
            display,
            stdout_is_tty: tty,
        }
    }

    /// Build a `ClipboardEnv` from a synthetic env var list (name, value).
    fn env_from(vars: &[(&str, &str)], tty: bool) -> ClipboardEnv {
        ClipboardEnv::from_env_lookup(
            |name| {
                vars.iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, v)| v.to_string())
            },
            tty,
        )
    }

    // ─── from_env_lookup ─────────────────────────────────────────────────────

    #[test]
    fn test_env_lookup_ssh_detected_from_any_ssh_var() {
        for var in ["SSH_TTY", "SSH_CONNECTION", "SSH_CLIENT"] {
            assert!(env_from(&[(var, "x")], true).ssh, "{var} should mark ssh");
        }
        assert!(!env_from(&[], true).ssh);
    }

    #[test]
    fn test_env_lookup_empty_ssh_var_does_not_count() {
        assert!(!env_from(&[("SSH_CONNECTION", "")], true).ssh);
    }

    #[test]
    fn test_env_lookup_gnu_screen_detected_via_sty_or_term() {
        assert!(env_from(&[("STY", "1234.pts-0")], true).screen);
        assert!(env_from(&[("TERM", "screen-256color")], true).screen);
        assert!(!env_from(&[("TERM", "xterm-256color")], true).screen);
    }

    #[test]
    fn test_env_lookup_tmux_with_screen_term_is_not_gnu_screen() {
        // tmux sets TERM=screen-256color by default; it must get raw OSC 52
        // (which tmux forwards), never screen's DCS wrapping (which tmux
        // would silently drop).
        let e = env_from(
            &[
                ("TERM", "screen-256color"),
                ("TMUX", "/tmp/tmux-1000/default,42,0"),
            ],
            true,
        );
        assert!(!e.screen);
        assert_eq!(e.osc52_mode(), Osc52Mode::Plain);
    }

    #[test]
    fn test_env_lookup_display_from_x11_or_wayland() {
        if cfg!(any(target_os = "macos", target_os = "windows")) {
            return; // display is unconditionally true on these platforms
        }
        assert!(env_from(&[("DISPLAY", ":0")], true).display);
        assert!(env_from(&[("WAYLAND_DISPLAY", "wayland-0")], true).display);
        assert!(!env_from(&[], true).display);
    }

    // ─── osc52_mode ──────────────────────────────────────────────────────────

    #[test]
    fn test_osc52_mode_plain_outside_screen() {
        assert_eq!(env(true, false, false, true).osc52_mode(), Osc52Mode::Plain);
    }

    #[test]
    fn test_osc52_mode_screen_inside_screen() {
        assert_eq!(env(true, true, false, true).osc52_mode(), Osc52Mode::Screen);
    }

    // ─── explicit modes ──────────────────────────────────────────────────────

    #[test]
    fn test_mode_off_is_disabled_everywhere() {
        let e = env(false, false, true, true);
        assert_eq!(
            choose_backend(ClipboardMode::Off, &e),
            BackendChoice::Disabled
        );
    }

    #[test]
    fn test_mode_system_never_falls_back() {
        let e = env(true, false, false, true);
        assert_eq!(
            choose_backend(ClipboardMode::System, &e),
            BackendChoice::System {
                osc52_fallback: None
            }
        );
    }

    #[test]
    fn test_mode_osc52_forced_even_without_tty() {
        let e = env(false, false, true, false);
        assert_eq!(
            choose_backend(ClipboardMode::Osc52, &e),
            BackendChoice::Osc52(Osc52Mode::Plain)
        );
    }

    #[test]
    fn test_mode_osc52_uses_screen_wrapping_inside_screen() {
        let e = env(false, true, false, true);
        assert_eq!(
            choose_backend(ClipboardMode::Osc52, &e),
            BackendChoice::Osc52(Osc52Mode::Screen)
        );
    }

    // ─── auto mode ───────────────────────────────────────────────────────────

    #[test]
    fn test_auto_ssh_prefers_osc52() {
        let e = env(true, false, false, true);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::Osc52(Osc52Mode::Plain)
        );
    }

    #[test]
    fn test_auto_ssh_with_x11_forwarding_still_prefers_osc52() {
        // DISPLAY is set via X forwarding, but the user's clipboard is on
        // their local machine — only OSC 52 reaches it.
        let e = env(true, false, true, true);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::Osc52(Osc52Mode::Plain)
        );
    }

    #[test]
    fn test_auto_ssh_inside_screen_uses_screen_wrapping() {
        let e = env(true, true, false, true);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::Osc52(Osc52Mode::Screen)
        );
    }

    #[test]
    fn test_auto_desktop_uses_system_with_osc52_fallback() {
        let e = env(false, false, true, true);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::System {
                osc52_fallback: Some(Osc52Mode::Plain)
            }
        );
    }

    #[test]
    fn test_auto_desktop_without_tty_has_no_osc52_fallback() {
        let e = env(false, false, true, false);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::System {
                osc52_fallback: None
            }
        );
    }

    #[test]
    fn test_auto_headless_tty_uses_osc52() {
        // Local console / headless box: no display server, stdout is a tty.
        let e = env(false, false, false, true);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::Osc52(Osc52Mode::Plain)
        );
    }

    #[test]
    fn test_auto_no_display_no_tty_is_disabled() {
        let e = env(false, false, false, false);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::Disabled
        );
    }

    #[test]
    fn test_auto_ssh_without_tty_falls_through_to_display_check() {
        // SSH session but stdout is redirected: OSC 52 cannot reach the
        // terminal, so fall back to the display-server path.
        let e = env(true, false, true, false);
        assert_eq!(
            choose_backend(ClipboardMode::Auto, &e),
            BackendChoice::System {
                osc52_fallback: None
            }
        );
    }
}
