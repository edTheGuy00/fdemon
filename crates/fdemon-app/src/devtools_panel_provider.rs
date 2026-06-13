//! Out-of-tree extension seam for host-injected DevTools panels.
//!
//! **Stability: UNSTABLE.** This is an embedder-facing API exercised by the
//! pro layer's preview features (widget-preview, live-preview). The method set
//! and context type may change without notice between minor versions. Stock
//! `fdemon` registers no panels, so this seam has zero effect on the public
//! binary's behaviour.
//!
//! # ratatui boundary
//!
//! **ratatui types (`Rect`, `Buffer`) appear ONLY in this module within
//! `fdemon-app`.** All other `fdemon-app` code must remain terminal-library-
//! independent. Do not import or use ratatui types anywhere else in this crate.
//!
//! # Overview
//!
//! DevTools mode renders four built-in panels (Inspector, Performance, Memory,
//! Network). An embedding host can register additional panels out-of-tree by
//! pushing [`DevToolsPanelProvider`] trait objects onto
//! [`crate::state::AppState::extra_devtools_panels`]. Registered panels appear
//! in the sub-tab bar after the four built-ins, participate in `Tab`/`Shift+Tab`
//! cycling, render their own content, and receive panel-scoped key events while
//! focused.
//!
//! This mirrors the [`crate::settings_tab_provider::SettingsTabProvider`]
//! registration/storage shape (a `Vec<Box<dyn _>>` field on `AppState`,
//! populated once by the embedder at startup) but exposes a **richer** trait:
//! settings tabs are pure data (title + items + save), whereas DevTools panels
//! own a render surface and handle keys.
//!
//! # Render contract
//!
//! [`render`](DevToolsPanelProvider::render) takes `&mut self` because stateful
//! widgets (e.g. a `ratatui-image` preview that caches a decoded protocol image)
//! need interior mutation during draw. The panel is handed the raw ratatui
//! [`Rect`] and [`Buffer`] plus a read-only [`DevToolsPanelCtx`]. Prefer mutating
//! the panel's own model over message-passing.
//!
//! # Redraw contract
//!
//! **The TUI redraws unconditionally on every event-loop iteration, which is
//! driven by a periodic ~50 ms `Tick` (see `fdemon-tui`'s runner).** ratatui's
//! double-buffer diff suppresses redundant terminal writes, so a panel whose
//! data changes out-of-band (network fetch, decoded frame, file watch) does
//! **not** need to signal a redraw — its next `render` call (within ~50 ms) will
//! pick up the new state automatically. Implementors may simply update their
//! in-memory model from a background task; no `request_redraw()` handle is
//! provided or required. This decision is intentional and the preview plans
//! defer to it: a stateful panel updates a field and the next tick paints it.

use crate::input_key::InputKey;
use crate::state::VmConnectionStatus;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// Outcome of routing a key event to a panel provider.
///
/// Returned by [`DevToolsPanelProvider::handle_key`] so the host knows whether
/// the panel consumed the key. Unconsumed keys are not currently re-dispatched
/// to global handlers (the host reserves `Tab`/`Shift+Tab`/`Esc` and the
/// built-in panel-switch letters before the provider ever sees a key), but the
/// signal keeps the contract explicit and forward-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    /// The panel consumed the key and may have mutated its own state.
    Consumed,
    /// The panel ignored the key.
    Ignored,
}

impl Handled {
    /// `true` when the variant is [`Handled::Consumed`].
    pub fn is_consumed(self) -> bool {
        matches!(self, Handled::Consumed)
    }
}

/// Read-only context passed to a panel on each render.
///
/// Intentionally minimal: it exposes the per-frame data a panel may need
/// without granting mutable access to engine state. Fields are additive — new
/// read-only context can be appended without breaking existing providers that
/// ignore them. It is marked `#[non_exhaustive]` so embedders construct it only
/// via the host, never by struct literal.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DevToolsPanelCtx {
    /// Whether the displayed session's VM Service is currently connected.
    ///
    /// `false` when no session is active. Stateful panels can use this to show
    /// a "waiting for connection" placeholder instead of stale data.
    ///
    /// This is a convenience alias for
    /// `connection_status == VmConnectionStatus::Connected`. Prefer
    /// `connection_status` when you need to distinguish `Reconnecting` or
    /// `TimedOut` from a clean `Disconnected`.
    pub vm_connected: bool,

    /// Full VM Service connection status for the displayed session.
    ///
    /// Richer than `vm_connected`: panels that animate reconnecting or timed-out
    /// states can branch on the variant rather than just the binary flag.
    /// Defaults to [`VmConnectionStatus::Disconnected`] when no session is active.
    pub connection_status: VmConnectionStatus,

    /// Monotonic animation frame counter, advanced once per ~50 ms tick.
    ///
    /// Panels that draw spinners or other time-based effects can derive a phase
    /// from this without reading wall-clock time, keeping `render` pure.
    pub animation_frame: u64,
}

impl DevToolsPanelCtx {
    /// Construct a render context.
    ///
    /// Host-internal: the TUI render path builds this from live `AppState` each
    /// frame. Embedders implementing panels receive a `DevToolsPanelCtx` in
    /// [`DevToolsPanelProvider::render`] and do not construct it themselves.
    pub fn new(vm_connected: bool, animation_frame: u64) -> Self {
        let connection_status = if vm_connected {
            VmConnectionStatus::Connected
        } else {
            VmConnectionStatus::Disconnected
        };
        Self {
            vm_connected,
            connection_status,
            animation_frame,
        }
    }

    /// Construct a render context with a full [`VmConnectionStatus`].
    ///
    /// Used by the TUI render path when the per-session `vm_connection_status`
    /// is available. `vm_connected` is derived from the status.
    pub fn with_status(connection_status: VmConnectionStatus, animation_frame: u64) -> Self {
        let vm_connected = connection_status == VmConnectionStatus::Connected;
        Self {
            vm_connected,
            connection_status,
            animation_frame,
        }
    }
}

/// A host-supplied DevTools panel.
///
/// Implementors own their in-memory model and render surface. The DevTools view
/// drives them through this trait without knowing anything about their content:
///
/// 1. [`id`](Self::id) is a stable identifier used to address the panel for
///    selection and `Tab` cycling. It must be unique across registered panels
///    and must not collide with the built-in ids (`inspector`, `performance`,
///    `memory`, `network`).
/// 2. [`title`](Self::title) is the sub-tab label.
/// 3. [`key_hint`](Self::key_hint) is a short footer string describing the
///    panel's keys (shown on the DevTools footer like the built-ins).
/// 4. [`render`](Self::render) draws the panel into `area` of `buf` using
///    `&mut self` so stateful widgets work.
/// 5. [`handle_key`](Self::handle_key) receives panel-scoped keys while the
///    panel is focused and returns whether it consumed them.
///
/// `Send + 'static` are required because the owning `AppState` crosses the async
/// boundary in the engine runner. `Sync` is intentionally **not** required:
/// unlike settings tabs (pure data), preview panels may hold `!Sync` interior
/// state (e.g. `Cell`-based render hints), and `AppState` is never shared across
/// threads — it is owned by the single engine task. The [`std::fmt::Debug`]
/// supertrait keeps `#[derive(Debug)]` on `AppState` working with the stored
/// `Vec<Box<dyn DevToolsPanelProvider>>` (mirrors `SettingsTabProvider`); a
/// `#[derive(Debug)]` on the panel struct satisfies it.
pub trait DevToolsPanelProvider: std::fmt::Debug + Send + 'static {
    /// Stable, unique identifier for this panel.
    ///
    /// Used as the addressable id for selection and cycling. Must not equal a
    /// built-in id (`inspector`, `performance`, `memory`, `network`).
    fn id(&self) -> &str;

    /// Sub-tab bar label.
    fn title(&self) -> &str;

    /// Short footer hint describing this panel's key bindings.
    ///
    /// Rendered on the DevTools footer when this panel is active, in the same
    /// slot the built-in panels use. Defaults to a generic exit hint.
    ///
    /// # Reserved keys
    ///
    /// The host reserves a number of keys at the global and DevTools levels
    /// before the provider ever sees them via [`handle_key`](Self::handle_key):
    ///
    /// - **Navigation / exit**: `Tab`, `Shift+Tab` (panel cycling), `Esc`
    ///   (exit DevTools), `q` (quit confirmation)
    /// - **Built-in panel switches**: `i` (Inspector), `p` (Performance),
    ///   `m` (Memory), `n` (Network)
    /// - **Global DevTools actions**: `b` (open browser DevTools), `r` (reload),
    ///   `R` (restart), `d` (debug overlays), and any other key bound to a
    ///   global or DevTools-level action in the key router
    ///
    /// Do not include these in your `key_hint` string and do not rely on
    /// receiving them in `handle_key`.
    fn key_hint(&self) -> &str {
        "[Esc] Logs  [Tab] Next Panel  [Shift+Tab] Prev Panel"
    }

    /// Render the panel into `area` of `buf`.
    ///
    /// Takes `&mut self` so stateful widgets can mutate their model during draw.
    /// `ctx` is read-only per-frame context. Implementors must not panic on any
    /// `area` size (including zero-sized); guard small areas defensively as the
    /// built-in panels do.
    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: DevToolsPanelCtx);

    /// Handle a panel-scoped key while this panel is focused.
    ///
    /// Called only when this panel is the active DevTools panel. The host
    /// intercepts navigation keys and all keys bound to global or DevTools-level
    /// actions before the provider sees them. This includes (but is not limited
    /// to): `Tab`/`Shift+Tab` (panel cycling), `Esc` (exit DevTools / close
    /// inner panel), the built-in panel-switch letters (`i`/`p`/`m`/`n`),
    /// global actions (`b`, `r`, `R`, `d`, `q`). Those keys are never delivered
    /// here. Return [`Handled::Consumed`] if the key was used.
    fn handle_key(&mut self, key: InputKey) -> Handled;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal stateful dummy panel used by registry tests across the crate.
    #[derive(Debug, Default)]
    struct DummyPanel {
        keys_seen: Vec<InputKey>,
        renders: usize,
    }

    impl DevToolsPanelProvider for DummyPanel {
        fn id(&self) -> &str {
            "dummy"
        }
        fn title(&self) -> &str {
            "Dummy"
        }
        fn render(&mut self, _area: Rect, _buf: &mut Buffer, _ctx: DevToolsPanelCtx) {
            self.renders += 1;
        }
        fn handle_key(&mut self, key: InputKey) -> Handled {
            self.keys_seen.push(key);
            Handled::Consumed
        }
    }

    #[test]
    fn handled_is_consumed_helper() {
        assert!(Handled::Consumed.is_consumed());
        assert!(!Handled::Ignored.is_consumed());
    }

    #[test]
    fn dummy_panel_renders_via_mut_self_and_records_keys() {
        let mut panel = DummyPanel::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 3));
        panel.render(Rect::new(0, 0, 10, 3), &mut buf, DevToolsPanelCtx::new(true, 0));
        assert_eq!(panel.renders, 1, "render must run via &mut self");

        let h = panel.handle_key(InputKey::Char('x'));
        assert_eq!(h, Handled::Consumed);
        assert_eq!(panel.keys_seen, vec![InputKey::Char('x')]);
    }

    #[test]
    fn default_key_hint_is_nonempty() {
        let panel = DummyPanel::default();
        assert!(!panel.key_hint().is_empty());
    }
}
