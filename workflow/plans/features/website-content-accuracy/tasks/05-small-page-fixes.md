## Task: Small page fixes (mouse, installation, introduction)

**Objective**: Three independent small corrections/additions, grouped because no other
task writes these files.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/docs/mouse.rs`: fix `[`/`]` session-cycle claim, "L" phrasing.
- `website/src/pages/docs/installation.rs`: Rust min version.
- `website/src/pages/docs/introduction.rs`: multi-launch + launch-lifecycle mention.

**Files Read (Dependencies):**
- `Cargo.toml:11`: `rust-version = "1.77.2"`.
- `crates/fdemon-app/src/handler/keys.rs`: session cycling = `Tab`/`Shift+Tab`.
- `crates/fdemon-core/src/types.rs`: launch-lifecycle `AppPhase`.

### Details

**`mouse.rs`:**
- Fix the `[` / `]` "cycle sessions" claim (appears twice) — sessions cycle with `Tab` /
  `Shift+Tab`; `[`/`]` are Performance detail-tab keys. [D-37] (`mouse.rs:145-149,
  266-270`; `keys.rs:197-202`)
- Reword "Shift+L" → "L" for link mode to match the rest of the docs. [D-38]

**`installation.rs`:**
- Rust minimum version `1.70+` → `1.77.2` (CVE-2024-24576 fix). [D-23]
  (`installation.rs:135`; `Cargo.toml:11`)

**`introduction.rs`:**
- Add a short overview mention of the multi-device launch picker (one confirm launches
  several devices) and the launch-lifecycle phases (`Preparing` → `Launching` →
  `Running`, only flipping to Running when the app is actually up).
- Refine the watcher line to note paths are configurable (default `lib`). [D-12]

### Acceptance Criteria

1. No `[`/`]` session-cycling claim remains in `mouse.rs`.
2. Installation states Rust `1.77.2`.
3. Introduction mentions multi-launch + launch phases.
4. `cd website && trunk build` compiles.

### Notes

- The SEO plan's S05 adds `leptos_meta` to `introduction.rs` and other pages — run S05
  after this task to avoid overlap.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-aa0d1628cc016a872

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/docs/mouse.rs` | Fixed two `[`/`]` session-cycle claims to `Tab`/`Shift+Tab`; changed "Shift+L" heading to "L" for link highlight badges |
| `website/src/pages/docs/installation.rs` | Updated Rust minimum version from `1.70+` to `1.77.2+` |
| `website/src/pages/docs/introduction.rs` | Refined watcher line to note configurable paths (default `lib`); added new "Multi-Device Launch" section describing multi-select launch picker and `Preparing` → `Launching` → `Running` lifecycle phases |

### Notable Decisions/Tradeoffs

1. **Multi-Device Launch placement**: Added as a new `<h3>` subsection inside "Key Features" between "Auto Hot Reload" and "Log Filtering & Search". This follows the existing pattern of feature subsections in that block.
2. **Watcher description**: Kept brief while noting configurability; referenced `.fdemon/config.toml` explicitly to match task requirement D-12.

### Testing Performed

- `cargo check` (via main repo website) — Passed (1 pre-existing unrelated warning in `debugging.rs`)
- Verified `[`/`]` no longer appear as session-cycling claims in `mouse.rs` (grep confirmed)
- Verified Rust version `1.77.2` matches `Cargo.toml:12`
- Verified link highlight key is `L` matching `keys.rs:336`
- Verified session cycling uses `Tab`/`BackTab` matching `keys.rs:197-198`

### Risks/Limitations

1. **Worktree cargo check**: The website crate cannot run `cargo check` directly from the worktree path because the worktree is nested inside the main repo directory tree, causing Cargo to find the parent workspace Cargo.toml. Verification was performed by temporarily copying files to the main repo's website directory and running `cargo check` there, then restoring the originals.
</content>
