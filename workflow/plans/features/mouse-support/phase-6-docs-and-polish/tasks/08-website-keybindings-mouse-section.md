## Task: Website Keybindings Page — Mouse Interactions Section

**Objective**: Add a "Mouse Interactions" `KeybindingSection` to the keybindings data model in `website/src/data.rs` so that the existing `Keybindings` page renders mouse mappings alongside keyboard mappings, without duplicating layout code.

**Depends on**: None

**Estimated Time**: 0.75h

### Scope

**Files Modified (Write):**
- `website/src/data.rs`: append a new `KeybindingSection` (or two, if scroll vs click are split for clarity) to the vector returned by `all_keybinding_sections()`. Each entry uses the existing `KeybindingSection` and `Keybinding` structs (no schema changes).

**Files Read (Dependencies):**
- `docs/MOUSE.md`: source of truth for mouse mappings.
- `docs/KEYBINDINGS.md`: cross-check that the keyboard mappings on the website page have not drifted from the docs.
- `website/src/pages/docs/keybindings.rs`: the page renderer; verify it iterates `all_keybinding_sections()` (it does — line 17–19 of the current file).

### Details

The keybindings page (`keybindings.rs`) renders one `KeybindingSectionView` per `KeybindingSection` in `data.rs::all_keybinding_sections()`. Each section has `title`, `color` (Tailwind bg), `key_color` (Tailwind text), and a list of `Keybinding { key, action, description }`.

Add at least one new section. Suggested split:

#### Option A — single "Mouse Interactions" section

```rust
KeybindingSection {
    title: "Mouse Interactions",
    color: "bg-pink-500",   // pick an unused color
    key_color: "text-pink-400",
    bindings: vec![
        Keybinding { key: "Wheel", action: "Scroll focused list/log", description: "Routes by current UiMode; coordinate-free" },
        Keybinding { key: "Shift+Wheel", action: "Page scroll", description: "Normal/LinkHighlight/DevTools-Network only; Windows 11 drops Shift modifier" },
        Keybinding { key: "Click [r]/[R]/[x]/[d]/[D]/[q]", action: "Hot reload / restart / stop / DevTools / DAP / quit", description: "Header bracketed shortcuts" },
        Keybinding { key: "Click tab", action: "Switch session", description: "Session tabs in multi-session mode" },
        Keybinding { key: "Middle-click tab", action: "Close session", description: "Closes the clicked session" },
        Keybinding { key: "Click device pill", action: "Open New Session dialog", description: "Single-session compact header only" },
        Keybinding { key: "Click log row", action: "Register for double-click", description: "No visible action on single click" },
        Keybinding { key: "Double-click log row", action: "Toggle stack trace", description: "Within 400 ms; entry_id-matched" },
        Keybinding { key: "Click DevTools sub-tab", action: "Switch panel", description: "Inspector / Performance / Network" },
        Keybinding { key: "Click Inspector row", action: "Select node", description: "Click expansion glyph to expand/collapse" },
        Keybinding { key: "Click frame bar", action: "Select frame", description: "Performance chart" },
        Keybinding { key: "Click Network row", action: "Select / refetch request", description: "Click detail tab to switch detail view" },
        Keybinding { key: "Click NewSessionDialog tab/device/field/Launch", action: "Activate", description: "Tab + device + field + launch button all clickable" },
        Keybinding { key: "Click ConfirmDialog Yes/No", action: "Confirm/cancel", description: "Bracket+label hit area" },
        Keybinding { key: "Click TagFilter row", action: "Toggle visibility", description: "Single click selects + toggles" },
        Keybinding { key: "Click LinkHighlight badge", action: "Open link", description: "Three-cell rect hit area" },
        Keybinding { key: "Click Settings row", action: "Select", description: "Double-click within 400 ms enters edit mode" },
    ],
},
```

#### Option B — two sections: "Mouse Scroll" and "Mouse Click"

If the Option A section feels too long, split into a "Mouse Scroll" section (the wheel/Shift+Wheel rows) and a "Mouse Click" section (everything else). Both sections share the same color theme (e.g. pink) so they read as a pair.

The author may pick A or B; A is the simpler default.

### Color Selection

Inspect existing sections for color usage (existing sections use blue, green, yellow, etc.). Pick a color that is not already in use. Suggestions: pink, indigo, amber, emerald. Match the bg/text suffix convention (`bg-<color>-500` / `text-<color>-400`).

### Acceptance Criteria

1. `data.rs::all_keybinding_sections()` returns at least one new section with `title` containing "Mouse" (e.g. "Mouse Interactions").
2. The new section's `bindings` cover at minimum: wheel scroll, Shift+wheel, header click, tab click, log double-click, DevTools sub-tab click, dialog button click, settings row click. Coverage of every entry from the PLAN.md interaction summary table is preferred.
3. The chosen color is not already in use by another section.
4. The page builds with `cd website && cargo check`.
5. Visual review: the keybindings page now scrolls to a "Mouse Interactions" section that renders with the same `<table>` styling as the keyboard sections.
6. No edits to `website/src/pages/docs/keybindings.rs` are required (the data-driven renderer picks up new sections automatically).
7. The new section's `Keybinding` rows have non-empty `description` fields where useful; empty descriptions are acceptable for self-evident rows.

### Testing

```bash
cd website && cargo check
# Trunk smoke test:
cd website && trunk serve --open
# Visit http://localhost:8080/docs/keybindings, scroll to Mouse Interactions section.
```

### Notes

- Do not add a new struct or enum; reuse `KeybindingSection` and `Keybinding` exactly. The whole point is data-driven uniformity.
- Do not edit the keybindings page renderer file — that is owned by Task 07 only if it ever needs modification (it likely does not).
- If you find an existing keyboard binding has drifted from `docs/KEYBINDINGS.md`, do not silently fix it in this task — file a separate small drift task and stay scoped.
- The mouse section's row count is bounded by readability. Aim for ≤ 20 rows in Option A; Option B's two-section split exists for cases where the table feels too long.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-abad90a9107b7c15a

### Files Modified

| File | Changes |
|------|---------|
| `website/src/data.rs` | Appended a new "Mouse Interactions" `KeybindingSection` with 17 bindings, using `bg-pink-500` / `text-pink-400` color theme |

### Notable Decisions/Tradeoffs

1. **Option A (single section)**: Used the single-section approach as the simpler default. 17 rows is within the ≤ 20 target.
2. **Color selection**: Used `bg-pink-500` / `text-pink-400`. All other colors (blue, cyan, green, orange, purple, red) were already taken by existing sections.
3. **Description quality**: All `description` fields are non-empty with useful context about timing windows, platform caveats, or equivalences to keyboard shortcuts.

### Testing Performed

- `cd website && cargo check` — Passed (1 pre-existing dead_code warning in debugging.rs, unrelated to this change)

### Risks/Limitations

1. **Trunk visual smoke test not run**: The task's trunk smoke test (`trunk serve --open`) was not run in this automated context. The `cargo check` pass confirms structural correctness; visual rendering depends on Tailwind's pink-500/pink-400 utility classes being present in the generated CSS — which they will be since Tailwind scans source files for class names at build time.
