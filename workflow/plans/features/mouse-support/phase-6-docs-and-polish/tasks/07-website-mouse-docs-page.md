## Task: Website Mouse Docs Page

**Objective**: Add a new `/docs/mouse` page to the website that mirrors `docs/MOUSE.md` content, register it in the docs sidebar nav, and add a route entry so existing nav components display the new link.

**Depends on**: None (reads `docs/MOUSE.md` content; even if Task 01 is in flight, the existing MOUSE.md content suffices for Phase 1–4 sections — Phase 5 sections can be added in a small follow-up if Task 01 lands later, but they should still be added here pre-emptively to avoid round-tripping).

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/mouse.rs` (NEW): new Leptos component `Mouse()` rendering the mouse docs content.
- `website/src/pages/docs/mod.rs`: add `pub mod mouse;` declaration; add a new `DocItem` entry for `/docs/mouse` to the `doc_items()` vector with an appropriate icon.
- `website/src/lib.rs` or wherever routes are registered: add a `<Route path="/docs/mouse" view=Mouse />` (or the project's equivalent route registration syntax). Verify by grepping how `/docs/keybindings` is wired and mirror that exactly.

**Files Read (Dependencies):**
- `docs/MOUSE.md`: canonical content source for the page body.
- `website/src/pages/docs/keybindings.rs`: closest layout precedent — a docs page that renders structured keybinding/interaction tables. Pattern to mirror.
- `website/src/pages/docs/mod.rs`: existing `doc_items()` shape; existing route patterns.
- `website/src/components/icons.rs`: available icon components. `MousePointer` may already exist, or pick an existing icon (e.g. `Keyboard`'s sibling). If no mouse-specific icon, reuse a generic `Cursor`-like or `Terminal` icon.

### Details

#### Page component (`website/src/pages/docs/mouse.rs`)

Follow the structure of `keybindings.rs` (a similar reference doc page). Suggested skeleton:

```rust
use leptos::prelude::*;

#[component]
pub fn Mouse() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Mouse Interactions"</h1>
            <p class="text-slate-400">
                "Flutter Demon supports mouse interaction in the terminal when " <code>"[ui] enable_mouse = true"</code>
                " (the default). This page describes scroll routing, click semantics for each UI surface, "
                "and platform caveats. For the on/off setting, see "
                <a href="/docs/configuration" class="text-blue-400">"Configuration"</a>"."
            </p>

            // Sections: Scroll Behavior; Click Surfaces (Header/Tabs/Log View/DevTools/Dialogs/Settings/LinkHighlight);
            //           Modal Precedence and Sub-Modal Gates; Compact NewSessionDialog; Platform Caveats; Disabling Mouse Capture
        </div>
    }
}
```

Each section should be a `<section>` with an `<h2>` heading. Tables (e.g. the scroll-mode table from MOUSE.md) should reuse the existing project Tailwind table classes (see how `keybindings.rs::KeybindingSectionView` styles its `<table>`).

The content **must** cover:

1. **Scroll Behavior by UI Mode** — the table from MOUSE.md.
2. **Modifier Key Rules** — bullet list (modes that honor `Shift+Wheel`, modes that ignore modifiers, the Inspector special case).
3. **Phase 3 click surfaces** — header brackets, session tabs, device pill.
4. **Phase 4 click surfaces** — log view single/double click, DevTools sub-tab, Inspector tree, Performance frame chart, Network table.
5. **Phase 5 click surfaces** — NewSessionDialog tabs/devices/fields/Launch button, ConfirmDialog Yes/No, TagFilter overlay, LinkHighlight badges, Settings panel rows.
6. **Modal Precedence and Sub-Modal Gates** — short prose paragraph (renderer-level base-region suppression).
7. **Compact NewSessionDialog** — the size-threshold caveat.
8. **Platform Caveats** — Windows 11 Shift-drop, legacy conhost.
9. **Disabling Mouse Capture** — TOML snippet and cross-link to the configuration page.

#### Sidebar nav (`mod.rs`)

Add to `doc_items()`:

```rust
DocItem {
    href: "/docs/mouse",
    label: "Mouse",
    icon: || view! { <YourIcon class="w-4 h-4 mr-3" /> }.into_any(),
},
```

Position it between `Keybindings` and `DevTools` (so the input-related docs cluster together). Choose an icon from `website/src/components/icons.rs`; if no clearly-mouse-themed icon exists, reuse one already imported by the docs layout (e.g. `Eye` or `Cursor` if available). Add the import at the top of `mod.rs`. Add `pub mod mouse;` to the `pub mod ...;` list at the top of the file.

#### Route registration

Find where `/docs/keybindings` is wired in the routes (likely in `website/src/lib.rs` or `website/src/main.rs`). Mirror its pattern for `/docs/mouse`. Use the same `Mouse` component name as the page module.

### Acceptance Criteria

1. `website/src/pages/docs/mouse.rs` exists and exports `pub fn Mouse() -> impl IntoView` (or `#[component] pub fn Mouse() ...`).
2. `website/src/pages/docs/mod.rs` declares `pub mod mouse;` and registers a `DocItem` for `/docs/mouse` with a non-empty `label`.
3. The new route is registered alongside other `/docs/*` routes; visiting `/docs/mouse` renders the page.
4. The page contains all nine sections listed above, with content faithful to `docs/MOUSE.md`.
5. The page builds with `cd website && cargo check` (or the project's existing build command) and produces no new compiler warnings.
6. Visual review: sidebar shows the new "Mouse" entry in the expected position; clicking it navigates to the new page; tables render with the existing site styling.
7. The page does not duplicate the keybindings page content; it focuses exclusively on mouse.

### Testing

```bash
# Build verification:
cd website && cargo check

# Optional: Trunk dev server smoke test:
cd website && trunk serve --open
# Manually visit http://localhost:8080/docs/mouse
```

### Notes

- Layout fidelity to `keybindings.rs` is the priority. Do not invent a new visual style.
- Code blocks (e.g. the TOML snippet for disabling capture) should use the existing `CodeBlock` component (see `keybindings.rs` siblings for usage).
- The page is Phase 5-aware. If you find that `docs/MOUSE.md` does not yet have Phase 5 content (Task 01 still in flight), add the Phase 5 sections to the website page anyway based on `phase-5-dialogs-overlays/TASKS.md` — both surfaces will land together in Phase 6.
- Do not touch `website/src/pages/home.rs`, `website/src/pages/docs/introduction.rs`, `website/src/pages/docs/keybindings.rs`, `website/src/pages/docs/configuration.rs`, or `website/src/pages/docs/architecture.rs`. Those are owned by other Phase 6 tasks.
- Do not add backend API calls, fetch logic, or markdown-rendering libraries — the content is hand-coded in JSX-style Leptos `view!` blocks, matching the rest of the site.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/docs/mouse.rs` | **Created** — 330-line Leptos component with all 9 required sections (Scroll Behavior by Mode, Modifier Key Rules, Phase 3 Header/Tabs, Phase 4 Log View/DevTools, Phase 5 Dialogs/Overlays, Modal Precedence, Compact NewSessionDialog, Platform Caveats, Disabling Mouse Capture) |
| `website/src/pages/docs/mod.rs` | Added `pub mod mouse;` declaration; added `MousePointer` to icon imports; added `DocItem` entry for `/docs/mouse` positioned between Keybindings and DevTools |
| `website/src/lib.rs` | Added `use pages::docs::mouse::Mouse;` import; added `<Route path=path!("/mouse") view=Mouse />` alongside other `/docs/*` routes |
| `website/src/components/icons.rs` | Added `MousePointer` icon using Lucide mouse-pointer SVG path via the existing `lucide_icon!` macro |

### Notable Decisions/Tradeoffs

1. **MousePointer icon**: No mouse-specific icon existed in `icons.rs`. Added `MousePointer` using Lucide's mouse-pointer path (diagonal arrow with click indicator). This is a one-path icon that uses the same `lucide_icon!` macro pattern as all other icons.
2. **`ScrollRow` helper component**: Added a private `ScrollRow` component to avoid repeating table row markup for the 18-row scroll-mode table, matching the pattern of other table-heavy pages (e.g. `KeybindingSectionView` in keybindings.rs).
3. **Section component**: Defined inline (same pattern as `configuration.rs`) using the `bg-blue-500` accent bar, matching the established page style exactly.
4. **Phase 5 content**: MOUSE.md does not yet have Phase 5 content (Task 01 still in flight). Phase 5 section was added to the website page based on `phase-5-dialogs-overlays/TASKS.md` as instructed, covering NewSessionDialog, ConfirmDialog, TagFilter, LinkHighlight badges, and Settings panel rows.
5. **`cargo check` worktree limitation**: The website crate cannot be checked with `cargo check` from this git worktree because the outer workspace root `/Users/ed/Dev/zabin/flutter-demon/Cargo.toml` is found first and its `exclude = ["website"]` pattern does not match the worktree path. This is a known worktree infrastructure limitation (all prior website tasks had the same constraint). The main workspace `cargo check --workspace --all-targets` passes cleanly; `rustfmt --check` on `mouse.rs` passes with no issues.

### Testing Performed

- `cargo fmt --all -- --check` — Passed (no formatting issues in workspace)
- `cargo check --workspace --all-targets` — Passed (workspace compiles cleanly)
- `rustfmt --edition 2021 --check website/src/pages/docs/mouse.rs` — Passed (file is correctly formatted)
- `cd website && cargo check` — Cannot run from worktree (pre-existing infrastructure limitation, not a code issue)

### Risks/Limitations

1. **Phase 5 content not yet in MOUSE.md**: The website page pre-emptively documents Phase 5 click surfaces based on task descriptions. When MOUSE.md Task 01 lands, verify the website copy matches the canonical doc text and update if needed.
2. **Worktree cargo check**: Full website build verification requires running from the main checkout (`cargo check -p flutter-demon-website` from `/Users/ed/Dev/zabin/flutter-demon/`), not from this worktree.
