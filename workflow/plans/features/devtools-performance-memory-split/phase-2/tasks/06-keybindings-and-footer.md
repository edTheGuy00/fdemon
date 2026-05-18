## Task: Document `]`/`[` Details Tab Cycling — KEYBINDINGS.md + Footer Hint

**Objective**: Add the Phase 2 `]` / `[` binding to the user-facing keymap documentation and surface the same hint in the in-app footer when the Performance panel is active. The hint string is the only user-visible reminder that the new binding exists.

**Depends on**: 03 (the binding has to exist before we document it)

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md` — add `]` and `[` rows under the Performance / DevTools section. If the doc already groups DevTools keys by panel, add a "Performance details" sub-section; otherwise append rows to the Performance table.
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — update the Performance arm of `render_footer` to reference `]/[`. Current string (around line 374–376):

  ```rust
  DevToolsPanel::Performance => {
      "[Esc] Logs  [←/→] Frames  [j/k] Scroll  [b] Browser  [Ctrl+p] PerfOverlay"
  }
  ```

  becomes:

  ```rust
  DevToolsPanel::Performance => {
      "[Esc] Logs  [←/→] Frames  [Tab] Section  [\u{005d}/\u{005b}] Tabs  [j/k] Scroll  [b] Browser"
      // Equivalently: "[Esc] Logs  [←/→] Frames  [Tab] Section  []/[] Tabs  ..."
  }
  ```

  Use raw `]` and `[` characters (no Unicode escape) in the actual string — the example above uses escapes only because plan-text rendering would otherwise eat the brackets. The footer is purely informational; it can mention every binding without conditional rendering.

**Files Read (Dependencies):**
- T03 task spec — the canonical source of which keys do what.
- `docs/KEYBINDINGS.md` (current layout — read before editing to match existing table / section conventions).
- `crates/fdemon-tui/src/widgets/devtools/mod.rs::render_footer` — current arm to edit.

### Details

#### KEYBINDINGS.md addition

The exact section to amend depends on the current doc layout — read `docs/KEYBINDINGS.md` first. The likely shape:

```markdown
### Performance panel

| Key | Action |
|-----|--------|
| `←` / `→` | Select previous / next frame |
| `Esc` | Deselect frame, or exit DevTools when no frame selected |
| `Tab` / `Shift+Tab` | Cycle focus between Frame Chart and Details pane |
| `]` | Cycle details tab forward (Frame Analysis → Rebuild Stats → Timeline Events) |
| `[` | Cycle details tab backward |
| `j` / `k` / `↑` / `↓` | Scroll the focused section |
| `Home` / `End` | Jump to oldest / live edge in the focused section |
| `b` | Open browser DevTools |
| `Ctrl+p` | Toggle Flutter performance overlay |
```

If the doc uses a single global table rather than per-panel sections, add `]` / `[` as new rows with a context column (e.g. `Performance (details focused)`).

Add a one-line note: `> The `]`/`[` cycle only fires when the Details pane has focus (press Tab from the Frame Chart). Frame Analysis is populated in Phase 2; Rebuild Stats and Timeline Events show a "Coming soon" stub until Phase 3.`

#### Footer hint format

Current Performance arm (≈ 65 chars):

```
[Esc] Logs  [←/→] Frames  [j/k] Scroll  [b] Browser  [Ctrl+p] PerfOverlay
```

Phase 2 target (≈ 80 chars):

```
[Esc] Logs  [←/→] Frames  [Tab] Section  []/[] Tabs  [j/k] Scroll  [b] Browser
```

The footer is truncated to `area.width - 2` by the existing renderer (see `render_footer`, line ~400), so the new string is safe to extend. If the truncation appears too aggressive on narrow terminals, drop `[b] Browser` first (lowest-value hint) — but Phase 2 should keep all five entries when possible.

### Acceptance Criteria

1. `docs/KEYBINDINGS.md` lists `]` and `[` with their action under the Performance panel section.
2. The doc explains that `]`/`[` cycling requires Details focus and that two of the three tabs are Phase 3 stubs.
3. The Performance arm of `render_footer` in `widgets/devtools/mod.rs` contains the substring `]/[` (or `[]/[]`) and `Tabs`.
4. Existing footer-string tests (if any) are updated to match the new substring expectations — search for `[Ctrl+p] PerfOverlay` in tests; that token disappears under the proposed string.
5. `cargo fmt --all -- --check && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` is green.

### Testing

If the footer string is covered by an existing test (search `crates/fdemon-tui/src/widgets/devtools/mod.rs` test module for `"PerfOverlay"`), update the assertion to look for the new substring `]/[` or `Tabs`. Otherwise no new test is required — the rendered text is exercised by the existing `render_footer` smoke tests.

```rust
#[test]
fn performance_footer_mentions_details_tab_cycling() {
    let mut state = DevToolsViewState::default();
    state.active_panel = DevToolsPanel::Performance;
    let s = footer_string(&state);
    assert!(s.contains("]/[") || s.contains("] /["), "footer was: {s}");
}
```

### Notes

- **No new key handler work**: T03 already routes `]` / `[`. T06 is documentation + footer hint only.
- **Footer hint must not split a bracket pair**: if width truncation lands mid-`]/[` the user sees `]/`. Truncation is by Unicode chars; for terminals narrower than ~70 columns the hint will already be truncated. Acceptable for Phase 2.
- **`Ctrl+p` PerfOverlay**: this binding is currently in the footer but actually toggles the Flutter performance overlay (debug extension). Whether to keep it in the footer post-Phase-2 is a judgement call — the proposed string drops it to make room for `]/[`. If the implementor disagrees, the alternative is to keep `Ctrl+p` and accept that the footer may be truncated on narrow terminals.
- **The `KEYBINDINGS.md` "DevTools" / "Performance" section**: if the doc has a hierarchical structure where Performance is a sub-section, add `]/[` there; if it's a flat global table, add a per-row context column. Match the doc's existing pattern — don't introduce a new convention.

---

## Completion Summary

(Filled in by implementor after work completes.)
