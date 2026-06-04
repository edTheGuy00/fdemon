## Task: Update ARCHITECTURE.md for the shared Android SDK-root resolver (M2 doc follow-up)

**Objective**: Reflect task 01's resolver consolidation in `docs/ARCHITECTURE.md`: the
daemon now owns a single `resolve_android_sdk_root_path` helper that both the install-time
path and the check-time `android_sdk_root()` derive from, and it is re-exported from the
`toolchain` module (and `fdemon-daemon` lib).

**Depends on**: 01

**Agent:** doc_maintainer

**Estimated Time**: 0.5 hours

### Background

Task 01 (M2) introduces `resolve_android_sdk_root_path(Option<&Path>) -> PathBuf` in
`crates/fdemon-daemon/src/toolchain/checks/android.rs`, re-exported via `checks/mod.rs`,
`toolchain/mod.rs`, and `lib.rs`, and deletes the duplicated private resolver in
`fdemon-app/src/actions/mod.rs`. `ARCHITECTURE.md` currently documents the toolchain
module's public surface and the `checks/` responsibilities; those references should mention
the shared resolver as the single source of truth for SDK-root resolution.

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — targeted edits only:
  - In the `toolchain/mod.rs` / `toolchain/checks/` module-reference rows, note the new
    `resolve_android_sdk_root_path` shared helper (install-time resolver) and that
    `android_sdk_root()` is now a thin `is_dir()`-filtering wrapper over it.
  - If the doc lists daemon re-exports, add `resolve_android_sdk_root_path`.

**Files Read (Dependencies):**
- Task 01's completion summary and the final code in `checks/android.rs`, `toolchain/mod.rs`,
  `lib.rs` (verify the exact symbol name and signature before documenting).

### Acceptance Criteria

1. `ARCHITECTURE.md` accurately describes the single shared SDK-root resolver and the
   wrapper relationship, matching the merged code (verify signature before writing).
2. Edits are targeted (no wholesale rewrite); content-boundary rules respected
   (architecture-only — no config values, no keybindings).
3. No stale reference to a duplicated app-side resolver remains.

### Notes

- Read `~/.claude/skills/doc-standards/schemas.md` for content boundaries before editing.
- Verify the symbol name/signature against the merged task 01 code — do not document the
  planned name if task 01 chose a different one.

---

## Completion Summary

**Status:**
**Branch:**

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
