## Task: Update Documentation for Windows Flutter Spawn Fix

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` and `docs/DEVELOPMENT.md` to reflect the new dependencies (`which`, `dunce`), the simplified `FlutterExecutable` semantics, the new Windows CI matrix, and the recommended Windows install hint (`[flutter] sdk_path` for shim-style installs).

**Depends on**: 02-simplify-flutter-executable, 03-locator-which-dunce, 04-diagnostic-error-paths

**Estimated Time**: 0.5-1h

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`:
  - In the SDK detection / locator section, mention that strategy 10 now uses the `which` crate for PATHEXT-aware lookup on Windows.
  - In the `FlutterExecutable` description, clarify that both variants invoke the absolute path directly via `Command::new`; the `WindowsBatch` discriminant is retained as a metadata marker only.
  - Add a note about `dunce::canonicalize` being preferred over `fs::canonicalize` anywhere a Windows path is later handed to `cmd.exe`.

- `docs/DEVELOPMENT.md`:
  - Add a "CI / Continuous Integration" subsection documenting the three-OS matrix (`ubuntu-latest`, `macos-latest`, `windows-latest`) and the `fmt + check + test + clippy` quality gate.
  - In "Common Issues", add a Windows-specific entry: "If `flutter devices` fails with 'The system cannot find the path specified.' on Windows, set `[flutter] sdk_path` in `.fdemon/config.toml`."
  - If the workspace MSRV was bumped to `1.77.2` (per task 02 notes), update the "Minimum Rust Version" line accordingly.

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules.
- `workflow/plans/bugs/windows-flutter-bat-spawn/BUG.md` — context for the change.
- All seven implementation task files (for the exact wording of each change).

### Change Context

This bug fix changes:

1. **Dependencies**: `which = "8"` and `dunce = "1"` added to `fdemon-daemon`. (DEVELOPMENT.md if dependency lists are kept there; ARCHITECTURE.md if module dependencies are listed there.)
2. **`FlutterExecutable` semantics**: Both variants now call `Command::new(path)` directly. The `WindowsBatch` variant no longer wraps `cmd /c`. (ARCHITECTURE.md — this is a public-ish API behavior.)
3. **Locator strategy 10**: Now uses `which::which("flutter")` instead of hand-rolled PATH walking. Strategy 11 (lenient) is unchanged in behavior. (ARCHITECTURE.md.)
4. **CI matrix**: Three-OS GitHub Actions workflow added. (DEVELOPMENT.md.)
5. **User-facing error message**: On Windows, when `flutter devices` fails, the error includes a hint pointing at `[flutter] sdk_path` in `.fdemon/config.toml`. (DEVELOPMENT.md "Common Issues".)

No new modules or layer dependencies; no new crates in the workspace tree. The architectural shape is unchanged. The doc updates are surgical clarifications, not rewrites.

### Acceptance Criteria

1. `docs/ARCHITECTURE.md` accurately describes the simplified `FlutterExecutable::command()` and the use of `which` in strategy 10.
2. `docs/DEVELOPMENT.md` documents the new CI workflow and adds the Windows-specific "Common Issues" entry.
3. No content boundary violations: architecture content stays in ARCHITECTURE.md (not DEVELOPMENT.md), build/CI content stays in DEVELOPMENT.md (not ARCHITECTURE.md).
4. All required sections per `~/.claude/skills/doc-standards/schemas.md` remain present.
5. Cross-references valid (especially links to the `Cargo.toml` and the `ci.yml` workflow file).
6. The Common Issues entry mentions the exact filename `.fdemon/config.toml` and the exact key `[flutter] sdk_path`.

### Notes

- Follow content boundaries strictly — see `~/.claude/skills/doc-standards/schemas.md`.
- Make targeted edits, do not rewrite entire documents.
- If `docs/ARCHITECTURE.md` does not currently describe the locator's 11 strategies in detail, do **not** add such a description — link to the source code instead. We don't want the doc to drift from the implementation.
- The user-facing CHANGELOG entry (if `CHANGELOG.md` exists at the repo root) is **out of scope** for `doc_maintainer`. The implementor or release manager updates the changelog separately.
- Do **not** edit `CLAUDE.md` — it's a Claude-Code instruction file, not project documentation.
- Do **not** edit `README.md` — keep this PR focused on internal docs.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-ac69a608db9b630bc

### Files Modified

| File | Changes |
|------|---------|
| `docs/DEVELOPMENT.md` | Updated MSRV from `1.70+` to `1.77.2` (line 9 and Common Issues entry); added "CI / Continuous Integration" subsection (three-OS matrix, quality gate); added Windows-specific "Common Issues" entry for `flutter devices` failure with `sdk_path` hint |
| `docs/ARCHITECTURE.md` | Added `flutter_sdk/` subtree to fdemon-daemon project structure; added `flutter_sdk/` files to fdemon-daemon module reference table; added `FlutterExecutable` variant table and semantics note (`WindowsBatch` as metadata marker, both variants use `Command::new(path)` directly, CVE-2024-24576 / MSRV 1.77.2 rationale) |

### Content Boundary Compliance

- All updates within correct document boundaries: YES
- Cross-contamination detected and fixed: N/A

### Notable Decisions/Tradeoffs

1. **FlutterExecutable description placement**: Placed the `FlutterExecutable` variant table and semantics note immediately after the module reference table in the fdemon-daemon section (before "Platform Support"), which keeps SDK detection information co-located with the module inventory it describes.
2. **Locator strategies not enumerated**: Per task notes, the 11 locator strategies are not listed in full — the doc links to source. Only the PATHEXT-aware change (strategy 10, `which::which`) is called out by name.
3. **Windows Common Issues entry uses `[flutter]` TOML header**: The exact key `[flutter] sdk_path` is referenced as specified in acceptance criteria, presented as a working TOML snippet rather than prose only.
