## Task: Update keybindings doc + website toolchain page for the version picker

**Objective**: Document the picker's user surface: new keys in `docs/KEYBINDINGS.md` and a version-
picker subsection on the website toolchain page (Flutter SDK step prose: any version/channel,
`~/fvm/versions/<version>`, git-only master, offline fallback).

**Depends on**: Tasks 01–05 (merged). Runs in parallel with Task 06 (write-disjoint).

**Agent:** implementor

**Complexity:** low

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `docs/KEYBINDINGS.md`
- `website/src/pages/docs/toolchain.rs`

**Files Read (Dependencies):**
- `phase-6/TASKS.md` Keyboard Shortcuts table (authoritative key list)
- `crates/fdemon-app/src/handler/keys.rs` — verify the merged key routing before documenting
- Phase-5 Task 06 completion notes — current structure/tone of the website toolchain page

### Details

#### `docs/KEYBINDINGS.md`

Add the install-wizard picker keys (match the existing table format):
`v` (FlutterSdk step → open picker), and the picker-mode block — `j`/`k`/`↓`/`↑` move, `Tab` channel
tab, `r` re-fetch, `Enter` confirm/install (error state: installs the default channel), `Esc` close.
Note that picker-visible interception suspends the underlying wizard keys, and that `Enter` on the
FlutterSdk step opens the picker when no version has been chosen yet.

#### `website/src/pages/docs/toolchain.rs`

In the Flutter SDK step section: a short "Choosing a version" subsection — the picker lists releases
from the official manifest grouped Stable / Beta / Master (git-only), defaults to the newest stable,
filters by CPU architecture on macOS; a pinned version installs to `~/fvm/versions/<version>`
alongside other versions (FVM-compatible) and appears in the Flutter Version panel; `master`/`main`
require git; without network the picker falls back to installing the configured channel. Mention the
precedence: a picker choice overrides `[toolchain] channel` for that run only. Update the step
keybinding hints shown on the page if it lists wizard keys.

### Acceptance Criteria

1. Every key in the TASKS.md shortcut table appears in KEYBINDINGS.md with matching semantics; no
   documented key contradicts the merged `keys.rs`.
2. The website page builds (`cargo check -p website` or the project's website build command — check
   `website/README`/`docs/DEVELOPMENT.md`) and the new prose matches merged behaviour (dir naming,
   git-only rule, offline fallback, precedence).
3. No edits to core docs (`ARCHITECTURE.md` etc. — Task 06 owns those).

### Notes

- Verify behaviour against merged code, not the task specs — Enter/`v` semantics were the most
  design-sensitive part of the phase.
- Keep the page's existing host-gating callout (Phase-5 N7 fix) intact.
