## Task: Website Configuration and Architecture Pages — Mouse Coverage

**Objective**: Add `enable_mouse` to the website Configuration page next to other `[ui]` settings, and add a short "Mouse Subsystem" mention to the website Architecture page that links to the new `/docs/mouse` page.

**Depends on**: None (read-only references to `docs/CONFIGURATION.md` and `docs/ARCHITECTURE.md`)

**Estimated Time**: 0.75h

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/configuration.rs`: add an `enable_mouse` row in the `[ui]` settings section (or whichever section pattern the page uses), with cross-link to `/docs/mouse`.
- `website/src/pages/docs/architecture.rs`: add a short "Mouse Subsystem" subsection (4–8 sentences) describing the registry pattern at a high level, with a cross-link to `/docs/mouse`.

**Files Read (Dependencies):**
- `docs/CONFIGURATION.md` lines 316/328/334: source of `enable_mouse` row content (default value, semantics, "Restart required" caveat, "When to disable" callout).
- `docs/ARCHITECTURE.md` Mouse Region Registry section (~lines 787–825): source of the architecture summary.
- `website/src/pages/docs/configuration.rs`: existing structure — find the `[ui]` section's settings table or list, mirror its row format.
- `website/src/pages/docs/architecture.rs`: existing structure — find the section breakdown (likely modules / TEA flow / DevTools); pick the most natural anchor for the Mouse Subsystem subsection.

### Details

#### Configuration page

Open `configuration.rs` and locate the `[ui]` settings rendering. Likely structures:

- A `<table>` with rows per setting (similar to the keybindings table style).
- A series of `<dl>` / `<dt>` / `<dd>` entries.
- Hand-coded `<div>` cards per setting.

Mirror whatever pattern is already used for, say, `theme` or `wrap_logs` (existing UI settings). Add a row for `enable_mouse` with:

- **Setting name:** `enable_mouse`
- **Type / default:** `bool` / `true`
- **Description:** "Enables terminal mouse capture for clickable UI surfaces. When false, fdemon does not emit mouse-capture escape sequences, leaving native terminal behavior (text selection, wheel scrollback) intact. Restart required after changing."
- **See also:** Link to `/docs/mouse` for the full mouse interaction reference.

If the page has a sibling "When to disable" callout block for any other setting, follow that visual pattern; otherwise just include the cross-link inline.

#### Architecture page

Open `architecture.rs` and identify the section structure. Add a new subsection titled "Mouse Subsystem" (or "Input — Mouse" if there's an existing "Input" subsection). The subsection text:

- Names the boundary types (`MouseInput`, `MouseRegions`, `MouseRegionsCell`, `MouseRegionGuard`).
- Briefly describes the per-frame registry-and-hit-test flow (widgets push regions during `view()`; click events run hit-tests against the registry).
- Mentions modal precedence (renderer suppresses base-UI region registration when a modal is up).
- Cross-links to `/docs/mouse` for user-facing semantics and to the existing source links if the page uses any (`ARCHITECTURE.md` itself has more depth).

Keep it short — the website architecture page is an overview, not a substitute for `docs/ARCHITECTURE.md`. Aim for 4–8 sentences plus links.

### Acceptance Criteria

1. `website/src/pages/docs/configuration.rs` has an `enable_mouse` row with: name, type/default (`bool` / `true`), description, and a link to `/docs/mouse`.
2. The `enable_mouse` row sits near other `[ui]` rows (theme, wrap_logs, etc.). If the page uses a `[ui]` heading, the row goes inside it; otherwise it goes wherever similar UI settings are listed.
3. `website/src/pages/docs/architecture.rs` has a "Mouse Subsystem" (or equivalent) subsection ≤ 8 sentences plus a link to `/docs/mouse`.
4. The architecture subsection mentions: `MouseInput`, the registry / hit-test flow, modal precedence. It does not duplicate the full content of `docs/ARCHITECTURE.md`.
5. `cd website && cargo check` succeeds, no new warnings.
6. Visual review: the configuration page now lists `enable_mouse`; the architecture page has the new subsection.

### Testing

```bash
cd website && cargo check
cd website && trunk serve --open
# Visit /docs/configuration → scroll to UI settings → confirm enable_mouse row.
# Visit /docs/architecture → confirm Mouse Subsystem subsection.
```

### Notes

- Do not duplicate the entire `docs/CONFIGURATION.md` content into the page. The website is summary + cross-link; canonical detail lives in the markdown.
- Do not add new layout primitives, components, or styles. Reuse existing patterns from the same page.
- The cross-links use SPA paths (`/docs/mouse`), not relative `MOUSE.md` paths.
- If `configuration.rs` uses generated content from `data.rs` (similar to how `keybindings.rs` does), the row may need to be added to `data.rs` instead. Check the imports at the top of `configuration.rs` and follow the data flow. (Note: if so, ensure no overlap with Task 08, which writes `data.rs` for keybindings — if both end up in `data.rs`, they touch different functions / vectors, so write overlap is safe but worth flagging in the completion summary.)
- If `architecture.rs` is highly visual (diagrams, terminal mockups), keep the new subsection text-only — do not introduce a new diagram in this task.
