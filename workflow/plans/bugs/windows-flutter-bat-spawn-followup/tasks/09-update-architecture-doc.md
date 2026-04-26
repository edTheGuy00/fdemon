## Task: Update `docs/ARCHITECTURE.md` for the follow-up changes

**Objective**: Reflect the implementation changes from Tasks 01, 04, and 06 in the project architecture documentation. Specifically: (1) the new `flutter_sdk/diagnostics.rs` shared module, (2) the new "Strategy 12: Binary-only fallback" in the locator's strategy chain, and (3) the stderr-content-gated hint behavior.

This is a `doc_maintainer`-routed task per the project's documentation policy — only the `doc_maintainer` agent edits `docs/ARCHITECTURE.md`, `docs/CODE_STANDARDS.md`, and `docs/DEVELOPMENT.md`.

**Agent**: doc_maintainer

**Depends on**: Task 01 (introduces `diagnostics.rs`), Task 04 (introduces Strategy 12), Task 06 (introduces hint gating + ANSI stripping) — Wave D

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`:
  - Update the `crates/fdemon-daemon` section's module list / file table to include `flutter_sdk/diagnostics.rs` (description: "Shared diagnostic helpers — `windows_hint()`, `is_path_resolution_error()`, `strip_ansi()`").
  - Update the `flutter_sdk/` section's strategy enumeration (currently 11 strategies after Wave-1) to add Strategy 12 with a one-line description: "Binary-only fallback for shim installers (scoop/winget); accepts a working `which::which` result when SDK-root inference fails."
  - Add a one-paragraph note about the stderr-gated `windows_hint()` behavior in the diagnostic surface section so future readers understand the hint is conditional.
  - If the doc currently mentions `cmd /c` anywhere as a current behavior (it shouldn't post-Wave-1), confirm it's removed.

**Files Read (Dependencies):**
- The post-merge state of `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` (created by Task 01, extended by Task 06).
- The post-merge state of `crates/fdemon-daemon/src/flutter_sdk/locator.rs` (Strategy 12 from Task 04, cached `try_system_path()` from Task 06).
- The post-merge state of `crates/fdemon-daemon/src/devices.rs` and `emulators.rs` (gated hint behavior from Task 06).
- `~/.claude/skills/doc-standards/schemas.md` (for content-boundary rules).

### Details

#### Module list update

Locate the `flutter_sdk/` module description in `docs/ARCHITECTURE.md` (added or modified by Wave-1 task 07). It should already mention `types.rs`, `locator.rs`, `windows_tests.rs`, and (added in Wave-1) the use of `which::which` and `dunce::canonicalize`.

Add a row to that module's file table for `diagnostics.rs`:

```markdown
| `flutter_sdk/diagnostics.rs` | Shared diagnostic helpers used by `devices.rs` and `emulators.rs` — `windows_hint()` (Windows-only, hints at `[flutter] sdk_path`), `is_path_resolution_error()` (stderr predicate to gate the hint), `strip_ansi()` (cleans Flutter CLI color codes from stderr before user-facing display). |
```

#### Strategy enumeration update

The locator's strategy chain should be documented somewhere in `ARCHITECTURE.md` (or be added if not present). Append Strategy 12:

```markdown
| 12 | Binary-only fallback (shim-installer support) | Last resort. When `which::which("flutter")` succeeds but the inferred SDK root fails both strict and lenient validation, returns a `FlutterSdk` with `source = SdkSource::PathInferred`, `version = "unknown"`. This unblocks scoop and winget Flutter installations that don't follow the canonical `<root>/bin/flutter` layout. |
```

If a corresponding paragraph elsewhere describes the strategy total count ("11 strategies"), update it to "12".

#### Diagnostic surface paragraph

Add (or amend) a short paragraph near the `flutter_sdk/diagnostics.rs` module entry explaining the gated-hint behavior:

```markdown
**Diagnostic hints are content-gated.** `devices.rs` and `emulators.rs` only
append the Windows-specific `windows_hint()` (which directs users to set
`[flutter] sdk_path` in `.fdemon/config.toml`) when the failure's stderr
matches a path-resolution error pattern (via `is_path_resolution_error()`).
This prevents the hint from misleading users when `flutter` exits non-zero
for unrelated reasons (e.g., adb crashed, license not accepted, network
proxy errors).
```

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` includes `flutter_sdk/diagnostics.rs` in the daemon module's file table with an accurate description of its three helpers.
2. The locator strategy enumeration (or equivalent prose) is updated to reflect 12 strategies, with Strategy 12 described as the binary-only shim-installer fallback.
3. A short paragraph explains the content-gated hint behavior so future readers understand the hint is conditional.
4. No content-boundary violations — the doc remains architectural prose only (no source code, no developer-workflow content).
5. The doc still passes the project's doc validation (if any automated check exists, e.g. via the `doc-standards` skill).

### Testing

```bash
# Manual: read the updated sections and confirm clarity
# If the project has a doc-validation skill or script:
# /doc-validate  (or equivalent)
```

### Notes

- Do NOT add implementation code, function signatures, or test-running commands to `docs/ARCHITECTURE.md` — those belong in `CODE_STANDARDS.md` (none here) or `DEVELOPMENT.md` (already updated in Wave-1, no further changes needed for this follow-up).
- Do NOT update `docs/DEVELOPMENT.md` in this task. The Wave-1 doc task covered the MSRV, CI matrix, and Common Issues. The follow-up doesn't introduce new build/test/run steps.
- Do NOT update `docs/CODE_STANDARDS.md`. No new conventions are introduced — the rename in `windows_tests.rs` (Task 05) brings the file into compliance with existing conventions, not new ones.
- If the existing `flutter_sdk/` description in `ARCHITECTURE.md` lists strategies inline (1-11), the addition of Strategy 12 is a one-line append. If it summarizes them as "PATH discovery (which::which)", you may need to expand that summary to mention the binary-only fallback path.
- Reference the source of truth: `crates/fdemon-daemon/src/flutter_sdk/locator.rs` (post-merge). If the strategy descriptions there have changed since Wave-1, mirror their current wording.
