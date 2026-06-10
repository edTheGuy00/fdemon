## Task: Update Documentation for the hardened iOS/macOS probe (Md1 doc reconciliation)

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` so the toolchain `checks/ios.rs` description matches the
**implemented** probe behavior after Task 01: a five-gate, read-only, non-interactive sequence
(`xcode-select -p` → `xcodebuild -version` → `xcodebuild -license check` → `xcodebuild
-checkFirstLaunchStatus` → `xcrun simctl list devices booted`), where `XcodeTools = Ok` requires **all** gates
to pass and a present-but-misconfigured Xcode reports `Missing` (capped to a non-blocking `Partial` at the
leaf). This replaces the prior over-claim that the probe checked `simctl`/license when it did not.

**Depends on**: Task 01 (merged) — the docs must describe the landed behavior.

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md` — the `toolchain/checks/` inline annotation (and the iOS/macOS probe description
  added in Phase 4) — targeted edits only.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- `crates/fdemon-daemon/src/toolchain/checks/ios.rs` (post-Task-01) — the authoritative probe sequence.
- The Phase 4 `ARCHITECTURE.md` annotations this updates.

### Change Context

1. **`checks/ios.rs` now performs real usability gates.** Previously documented (Phase 4) / claimed as
   probing `simctl` + license; the original Phase 4 implementation actually ran only `xcode-select -p` +
   `xcodebuild -version`. Task 01 implements the missing gates. Update the annotation to describe the real
   sequence and the `Ok`-iff-all-pass rule.
2. **`kill_on_drop` + non-zero-exit fix** are implementation hardening — mention only if the existing
   annotation describes the probe's process handling; do not add new detail beyond the schema's allowed
   altitude.
3. **Non-blocking semantics unchanged** — the `Missing → Partial` cap and the `FlutterSdk`-only handback gate
   are exactly as Phase 4 documented; do not restate them beyond a cross-reference.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately describes the implemented five-gate `checks/ios.rs` probe and the
   `XcodeTools = Ok` ⇔ all-gates-pass rule; no remaining claim that license/simctl are merely inferred or
   unimplemented.
2. Targeted edits to the existing inline annotation — no whole-section rewrite.
3. No content-boundary violations (architecture content only; no how-to/command tutorials).

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- **`docs/CONFIGURATION.md` / `docs/KEYBINDINGS.md` need no change** (no new config field, no new keybinding).
- **Website docs stay deferred** to the Phase-5 wrap-up (per the Phase-2/3/4 notes).
- The Phase-4 planning `TASKS.md` over-claim is historical; do not edit prior plan files — the corrected
  behavior lives in this follow-up plan + `ARCHITECTURE.md`.

---

## Completion Summary

**Status:** Not Started
