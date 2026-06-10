## Task: Update Documentation for the Flutter SDK version picker

**Agent:** doc_maintainer

**Objective**: Update `docs/ARCHITECTURE.md` to reflect the Phase 6 version-picker additions across
all three layers.

**Depends on**: Tasks 01–05 (merged).

**Complexity:** low

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `docs/ARCHITECTURE.md`

**Files Read (Dependencies):**
- `~/.claude/skills/doc-standards/schemas.md` — content boundary rules
- Task files 01–05 (this directory) + completion summaries for final naming

### Change Context

1. **fdemon-daemon / toolchain**: `FlutterRelease` now carries `release_date`;
   `FlutterInstallTarget.version_tag` selects an exact version (git `-b <tag>` shallow clone;
   archive resolves via the new private `resolve_version_release`, hard-error on a manifest miss —
   the silent stable fallback applies to channel installs only); ref validation accepts `+`
   (old hotfix tags).
2. **fdemon-app / install_wizard**: new `install_wizard/version_picker.rs` state module
   (`VersionPickerState`: lazy manifest fetch, Stable/Beta/Master tabs with synthetic git-only
   rows, host-arch filtering, confirmed selection) and new `handler/install_wizard/
   version_picker.rs` handler module; `UpdateAction::FetchFlutterReleaseManifest` +
   `FlutterManifestFetched/FetchFailed` follow the preflight spawn→message pattern;
   `FlutterStepParams.version_tag` threads the pick to the executor, which maps it to
   `version_dir_name` so pinned installs land at `~/fvm/versions/<version>`. Precedence: picker
   choice > `settings.toolchain.channel`, per-run only, never persisted.
3. **fdemon-tui / widgets/install_wizard**: new `version_picker.rs` nested overlay (rendered inside
   `InstallWizardPanel`, tag-filter-style key interception while visible), FlutterSdk step
   caption/hint updates.

### Acceptance Criteria

1. The install-wizard section of ARCHITECTURE.md describes the picker's state/handler/widget split
   and the version-tag data flow (picker → params → target → `~/fvm/versions/<version>` → FVM cache
   scan), matching the merged code.
2. The toolchain/daemon section records `version_tag` semantics and the no-fallback rule for pinned
   versions.
3. No content boundary violations; targeted edits only (no rewrite); cross-references valid.

### Notes

- Follow content boundaries strictly — keybindings belong to `docs/KEYBINDINGS.md` (Task 07), not
  here.
- Verify final symbol names against the merged code before writing — task files are the plan, not
  the implementation record.
