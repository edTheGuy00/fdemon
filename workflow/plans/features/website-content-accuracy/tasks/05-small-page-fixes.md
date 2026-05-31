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
</content>
