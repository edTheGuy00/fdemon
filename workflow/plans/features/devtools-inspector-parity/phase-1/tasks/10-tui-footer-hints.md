## Task: Update DevTools footer hint string for tree vs details mode

**Objective**: The Inspector footer string at the bottom of the panel currently reads `"[Esc] Logs  [↑↓] Navigate  [→] Expand  [←] Collapse  [r] Refresh  [b] Browser"`. After Phase 1 it should reflect the two modes:

- Tree mode: `[Esc] Logs  [↑↓] Navigate  [→] Expand  [←] Collapse  [Enter] Details  [Shift+H] Hide Impl  [r] Refresh  [b] Browser`
- Details mode: `[Esc] Close  [Tab] Next Tab  [Shift+Tab] Prev Tab  [r] Refresh  [b] Browser`

**Depends on**: 02-state-inspector-extensions

**Estimated Time**: ~1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` (read `inspector.details_open`).

### Details

#### 1. Locate the footer

The footer is rendered at `crates/fdemon-tui/src/widgets/devtools/mod.rs:347–350` in the `render_footer` function. The current `match` arm for `DevToolsPanel::Inspector` returns a single static string. Change it to a function or `match` that includes `details_open`.

#### 2. Add the branching

```rust
DevToolsPanel::Inspector => {
    if state.devtools_view_state.inspector.details_open {
        "[Esc] Close  [Tab] Next Tab  [Shift+Tab] Prev Tab  [r] Refresh  [b] Browser"
    } else {
        "[Esc] Logs  [↑↓] Navigate  [→] Expand  [←] Collapse  [Enter] Details  [Shift+H] Hide Impl  [r] Refresh  [b] Browser"
    }
}
```

If `render_footer` doesn't have access to `inspector.details_open`, plumb it through (the `state: &AppState` parameter should already give access).

#### 3. Length check

Terminal widths can be as narrow as 80 cols. The tree-mode hint above is ~95 cols. If that's wider than available, truncation already kicks in (the existing footer rendering truncates or wraps). Verify the existing behavior — if the footer doesn't truncate gracefully, abbreviate:

- `[↑↓] Nav`, `[→] Exp`, `[←] Col`, `[Enter] Det`, `[Shift+H] Hide`, `[r] Ref`, `[b] Brw` — only as a fallback for narrow terminals.

For Phase 1, keep the full-length string. If the footer renderer truncates, that's acceptable for narrow terminals.

#### 4. Tests

If there are existing footer-snapshot tests (search `render_footer` / `footer` in tests files), update them to cover both modes. If none exist, add minimal tests:

- `inspector_footer_in_tree_mode_includes_enter_details_hint`.
- `inspector_footer_in_details_mode_includes_esc_close_hint`.
- `inspector_footer_in_details_mode_does_not_include_navigate_hint`.

### Acceptance Criteria

1. Footer text differs between tree mode and details mode at runtime.
2. Tree-mode footer includes `[Enter] Details` and `[Shift+H] Hide Impl`.
3. Details-mode footer includes `[Esc] Close`, `[Tab] Next Tab`, `[Shift+Tab] Prev Tab`.
4. `cargo test -p fdemon-tui` passes; existing footer tests are not regressed.
5. `cargo clippy -p fdemon-tui --all-targets -- -D warnings` passes.

### Testing

```rust
#[test]
fn inspector_footer_in_tree_mode_includes_enter_details_hint() {
    let state = make_state_in_devtools_inspector(/* details_open = */ false);
    let s = footer_string(&state);
    assert!(s.contains("[Enter] Details"), "footer was: {s}");
    assert!(s.contains("[Shift+H] Hide Impl"), "footer was: {s}");
}

#[test]
fn inspector_footer_in_details_mode_includes_esc_close_hint() {
    let state = make_state_in_devtools_inspector(true);
    let s = footer_string(&state);
    assert!(s.contains("[Esc] Close"));
    assert!(s.contains("[Tab] Next Tab"));
    assert!(!s.contains("[↑↓] Navigate"), "navigate hint should be hidden in details mode");
}
```

### Notes

- This task is purely cosmetic but user-visible — wrong/stale hints frustrate users. Coordinate the exact wording with the key bindings task (06) so they agree.
- Other DevTools tabs (Performance, Network) keep their existing footer strings unchanged.
- Document the new footer strings in `docs/KEYBINDINGS.md` (task 11).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a0e2743f6fe01690a

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Updated `render_footer` Inspector arm to branch on `details_open`; added `make_state_in_devtools_inspector`, `footer_string` test helpers and three new tests |

### Notable Decisions/Tradeoffs

1. **footer_string helper uses 200-column width**: The tree-mode hint is ~95 characters, so an 80-column test buffer would truncate it and break assertions. Using width=200 avoids truncation without changing any production code.
2. **Test helper reads row y=23 (last row of a 24-row buffer)**: The DevTools layout splits the area into a 3-row tab bar + 21-row panel content; `render_footer` writes to `panel.y + panel.height - 1 = 23`. This is deterministic for the fixed 24-row test area.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test -p fdemon-tui` - Passed (1061 tests)
- `cargo clippy -p fdemon-tui --all-targets -- -D warnings` - Passed
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Narrow terminal truncation**: The tree-mode hint (~95 chars) is truncated on terminals narrower than ~97 cols. The existing truncation logic (`hints.chars().take(max_width)`) handles this gracefully — no visual corruption, just shorter hints. This is acceptable per the task spec.
