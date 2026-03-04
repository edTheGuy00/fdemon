## Task: Status Bar DAP Badge and Header Keybinding Hints

**Objective**: Add a `[DAP :PORT]` badge to the status bar when the DAP server is running, and add `[D] DAP` to the header keybinding hints in Normal mode.

**Depends on**: 03 (DapStatus on AppState)

### Scope

- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — Add `dap_port` field to `StatusInfo`, render `[DAP :PORT]` badge
- `crates/fdemon-tui/src/render/mod.rs` — Populate `dap_port` in `StatusInfo` constructor
- `crates/fdemon-tui/src/widgets/header.rs` — Add `[D] DAP` hint to shortcuts
- `crates/fdemon-tui/Cargo.toml` — May need `fdemon-app` for `DapStatus` access (already a dependency)

### Details

#### 1. StatusInfo Field (`widgets/log_view/mod.rs`)

Add a new field to `StatusInfo` (at line 45, after `vm_connected`):

```rust
pub struct StatusInfo<'a> {
    pub phase: &'a AppPhase,
    pub is_busy: bool,
    pub mode: Option<&'a FlutterMode>,
    pub flavor: Option<&'a str>,
    pub duration: Option<Duration>,
    pub error_count: usize,
    pub vm_connected: bool,
    /// DAP server port if running (shows [DAP :PORT] badge).
    pub dap_port: Option<u16>,
}
```

#### 2. DAP Badge Rendering (`widgets/log_view/mod.rs`)

In `render_bottom_metadata()`, directly after the `[VM]` badge block (line 841), add the DAP badge:

```rust
// DAP server indicator
if let Some(port) = status.dap_port {
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        format!("[DAP :{port}]"),
        Style::default().fg(palette::STATUS_GREEN),
    ));
}
```

This renders in the same style as the `[VM]` badge, consistent with the existing visual language. The badge only appears when `dap_port` is `Some` (i.e., when the DAP server is running).

**Compact mode**: The DAP badge follows the same compact-mode guard as `[VM]` — it only renders in the full (non-compact) metadata row. This is controlled by the existing `if !compact` branch in `render_bottom_metadata()` (line 816). No additional compact-mode logic is needed.

#### 3. StatusInfo Construction (`render/mod.rs`)

In the `StatusInfo` constructor (lines 97-109), populate the new field from `AppState.dap_status`:

```rust
let status_info = StatusInfo {
    phase: &handle.session.phase,
    is_busy: handle.session.is_busy(),
    mode: handle.session.mode.as_ref(),
    flavor: handle.session.flavor.as_deref(),
    duration: handle.session.duration(),
    error_count: handle.session.error_count,
    vm_connected: handle.session.vm_connected,
    dap_port: state.dap_status.port(), // DapStatus::port() returns Option<u16>
};
```

Note: `dap_status` is on `AppState` (global), not per-session. The DAP server serves all sessions.

#### 4. Header Keybinding Hint (`widgets/header.rs`)

In `render_title_row()` (lines 166-182), add `[D] DAP` to the shortcuts vector. Insert before the `[q] Quit` group:

```rust
// Before [q] Quit, add [D] DAP:
Span::styled("[", Style::default().fg(palette::TEXT_MUTED)),
Span::styled("D", Style::default().fg(palette::STATUS_YELLOW)),
Span::styled("] DAP  ", Style::default().fg(palette::TEXT_MUTED)),
```

The three-span pattern matches all existing hints. The trailing `"  "` in `"] DAP  "` provides consistent spacing before the next hint.

#### 5. Update Existing Tests

**Status bar tests**: If there are existing tests that construct `StatusInfo`, add the new `dap_port: None` field (or `dap_port: Some(4711)` for DAP-enabled tests).

**Header tests**: The test at line 383 (`test_header_with_keybindings`) asserts on hint strings. Add an assertion for `"[D] DAP"` presence.

### Acceptance Criteria

1. `StatusInfo` has a `dap_port: Option<u16>` field
2. When `dap_port` is `None`, no DAP badge is rendered (default state)
3. When `dap_port` is `Some(4711)`, `[DAP :4711]` badge appears after `[VM]` badge
4. Badge uses `palette::STATUS_GREEN` style (same as `[VM]`)
5. Badge has a two-space separator before it (consistent with `[VM]`)
6. Badge only renders in non-compact mode (terminals >= `MIN_FULL_STATUS_WIDTH` columns)
7. `dap_port` is populated from `state.dap_status.port()` in `render/mod.rs`
8. Header shows `[D] DAP` hint in Normal mode
9. `D` key character uses `STATUS_YELLOW` style (same as other hint keys)
10. Header hint `[D] DAP` appears before `[q] Quit` in the shortcuts list
11. All existing tests still pass (no regressions from added field)
12. New tests cover DAP badge rendering (with port, without port)
13. `cargo check -p fdemon-tui` passes
14. `cargo test -p fdemon-tui` passes
15. `cargo clippy -p fdemon-tui -- -D warnings` clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_no_dap_badge_when_off() {
        let status = StatusInfo {
            // ... other fields ...
            dap_port: None,
        };
        // Render and verify no "[DAP" substring in output
    }

    #[test]
    fn test_status_bar_shows_dap_badge_with_port() {
        let status = StatusInfo {
            // ... other fields ...
            dap_port: Some(4711),
        };
        // Render and verify "[DAP :4711]" appears in output
    }

    #[test]
    fn test_status_bar_dap_badge_different_port() {
        let status = StatusInfo {
            // ... other fields ...
            dap_port: Some(54321),
        };
        // Render and verify "[DAP :54321]" appears
    }

    #[test]
    fn test_header_shows_dap_hint() {
        // Render header and verify "[D] DAP" appears in shortcuts
    }

    #[test]
    fn test_dap_badge_hidden_in_compact_mode() {
        // Render with small area (compact=true), verify no DAP badge
    }
}
```

### Notes

- The DAP badge is intentionally simple in Phase 2 — just `[DAP :PORT]`. Phase 4 could add a connected indicator (e.g., `[DAP :4711 ●]` when clients are connected) using `dap_status.client_count() > 0`.
- The `[D] DAP` header hint is always shown in Normal mode, regardless of whether the DAP server is currently running. This matches how `[d] DevTools` is always shown even when not in DevTools mode — it's a hint about available actions, not current state.
- `dap_port` comes from `state.dap_status.port()` which is `Some(u16)` only when `DapStatus::Running`. During `Starting`/`Stopping` states, it returns `None` and no badge is shown. This is correct — the badge should only show the port when the server is actively listening.
- Consider whether `dap_port: Option<u16>` or a richer `dap_status: &DapStatus` reference is better for `StatusInfo`. The simpler `Option<u16>` is sufficient for Phase 2 rendering and avoids coupling `fdemon-tui` to the full `DapStatus` enum.
