# Code Review: DAP Server Phase 5 — IDE DAP Auto-Configuration

**Date:** 2026-03-09
**Branch:** `feat/dap-server`
**Scope:** 12 tasks across 4 waves (~1,137 lines added, 16 modified files, 7 new files)
**Verdict:** NEEDS WORK

---

## Summary

Phase 5 adds automatic IDE DAP configuration generation. When the DAP server starts, fdemon detects the parent IDE and writes the appropriate config file (launch.json, languages.toml, debug.json, or dap-emacs.el). Also adds a `--dap-config <IDE>` CLI flag for manual/standalone generation and a TUI badge showing which IDE's config was generated.

The architecture is sound — layer boundaries are respected, the TEA pattern is followed correctly, and the `IdeConfigGenerator` trait design is clean with pure generators separated from I/O. Test coverage is thorough across all 5 IDE generators. However, there is one functional gap (CLI override not wired through), one code duplication liability, and several minor issues that should be addressed.

---

## Agent Verdicts

| Agent | Verdict | Key Finding |
|-------|---------|-------------|
| Architecture Enforcer | WARNING | Layer boundaries PASS; `--dap-config` override not threaded; `ConfigAction::Skipped` dead code |
| Code Quality Inspector | NEEDS WORK | `--dap-config` override bug; `indoc` no-op; duplicated JSONC parser; unsafe env var tests |
| Logic Reasoning Checker | CONCERNS | CLI override silently dropped; unconditional file writes; Emacs merge path regression |
| Risks/Tradeoffs Analyzer | CONCERNS | 1 HIGH risk (override gap), 2 MEDIUM (Emacs path, code duplication), 3 LOW |

---

## Issues

### Critical / Must Fix

#### 1. `--dap-config` CLI override silently dropped in combined mode
**Flagged by:** All 4 agents
**Files:** `src/main.rs:130-138`, `crates/fdemon-app/src/handler/dap.rs:74-76`

When `--dap-config vscode` is used without `--dap-port`, the IDE name is validated early but the parsed `ParentIde` value is discarded. The comment at `main.rs:132-133` claims "the IDE override is threaded through the action when the DAP server starts" — this is false. `handle_started()` always emits `GenerateIdeConfig { ide_override: None }`, causing silent fallback to environment detection.

The `UpdateAction::GenerateIdeConfig` has the `ide_override` field ready, but nothing populates it in the combined-mode path. A user running `fdemon --dap-config neovim` in a plain terminal (no `$NVIM` set) will get no config generated.

**Fix:** Store the parsed `ParentIde` on `Engine` or `AppState` (e.g., `cli_dap_config_override: Option<ParentIde>`), populate it from CLI args during initialization, and pass it through `handle_started()` to the action. Alternatively, remove the misleading comment and warn the user that combined mode is not yet implemented.

---

### Major / Should Fix

#### 2. `clean_jsonc()` duplicated across two modules (~120 lines)
**Flagged by:** Architecture, Code Quality, Risks
**Files:** `crates/fdemon-app/src/ide_config/merge.rs:67-128`, `crates/fdemon-app/src/config/vscode.rs`

`strip_json_comments`, `strip_trailing_commas`, and `clean_jsonc` are byte-for-byte copies. The comment says "duplicated to keep modules independent" but both modules live in the same crate — `pub(crate)` would resolve this with zero API coupling.

**Fix:** Extract to a shared location (e.g., `crate::config::jsonc` or `crate::util::jsonc`) and import from both call sites.

#### 3. Emacs `merge_config` produces degraded output (relative placeholder path)
**Flagged by:** Code Quality, Logic, Risks
**File:** `crates/fdemon-app/src/ide_config/emacs.rs:57-59`

On the merge path (file already exists), `merge_config` calls `generate_elisp(port, ".fdemon/dap-emacs.el")` with a relative placeholder because `project_root` is unavailable in the trait signature. The `generate()` path correctly uses the absolute path. Users who trigger an update on a DAP restart get degraded loading instructions.

**Fix:** In `run_generator`, when the file already exists for Emacs, call `generate(port, project_root)` instead of `merge_config()` since Emacs "merge" is semantically identical to "overwrite." Avoids changing the trait signature.

#### 4. `run_generator` writes unconditionally (no content comparison)
**Flagged by:** Logic, Risks, Architecture
**File:** `crates/fdemon-app/src/ide_config/mod.rs:138-150`

Every DAP server start triggers a file write even when content is identical. This causes unnecessary git diffs, modified timestamps, and `ConfigAction::Updated` reported when nothing changed. The `ConfigAction::Skipped` variant exists but is never produced (dead code).

**Fix:** Compare new content against existing file content before writing. Return `ConfigAction::Skipped("content unchanged")` when identical.

#### 5. `indoc()` is a no-op with misleading doc comment
**Flagged by:** Code Quality
**File:** `crates/fdemon-app/src/ide_config/helix.rs:162-164`

```rust
/// Strip a leading `\n` from a string literal used with `indoc!`-style indentation.
fn indoc(s: &str) -> String {
    s.to_string()
}
```

The function does nothing — it just calls `.to_string()`. The doc comment describes behavior that doesn't exist.

**Fix:** Remove the function and call `.to_string()` directly on the raw string, or add the `indoc` crate and use `indoc::indoc!`.

---

### Minor / Consider Fixing

#### 6. Unsafe env var mutation in parallel tests
**File:** `crates/fdemon-app/src/config/settings.rs:1835-1858`

`set_var`/`remove_var` are marked `unsafe` (Rust 2024 edition) but are still unsound in parallel test execution. The `// SAFETY:` comments are incorrect — other test threads may observe the mutated env vars.

**Fix:** Use `#[serial]` from the `serial_test` crate, or inject env reading through a parameter/closure.

#### 7. Public re-exports of internal merge utilities
**File:** `crates/fdemon-app/src/ide_config/mod.rs:254-257`

`clean_jsonc`, `find_json_entry_by_field`, etc. are re-exported as public API of `fdemon-app`. These are implementation utilities, not public API.

**Fix:** Make them `pub(crate)` in `merge.rs` and remove the `pub use` block.

#### 8. `unreachable!()` in helix merge after verified insert
**File:** `crates/fdemon-app/src/ide_config/helix.rs:207`

Panics if the TOML library behaves unexpectedly after an insert.

**Fix:** Replace with a typed `Error::config(...)` return.

#### 9. Redundant `is_some()` + branch pattern
**File:** `crates/fdemon-app/src/actions/mod.rs:604-605`

```rust
let ide = if ide_override.is_some() { ide_override } else { detect_parent_ide() };
```

**Fix:** `let ide = ide_override.or_else(|| detect_parent_ide());`

#### 10. Zed uses `"Delve"` adapter type (Go debugger) for Dart/Flutter
**File:** `crates/fdemon-app/src/ide_config/zed.rs:44`

Semantically incorrect but currently functional. May break if Zed validates adapter types.

**Fix:** Add a comment in the generated `debug.json` noting this is a workaround.

---

## Strengths

- **Clean trait design**: `IdeConfigGenerator` with pure content generation separated from file I/O in `run_generator()` — excellent testability
- **Layer boundaries**: All respected. No downward dependencies. `ide_config/` correctly placed in `fdemon-app`
- **TEA compliance**: Config generation runs as async `UpdateAction`, results flow back via `Message::DapConfigGenerated` — textbook TEA
- **Thorough testing**: Edge cases covered (malformed files, empty files, merge preservation, port substitution, balanced parens in Elisp)
- **Error resilience**: Merge parse errors are caught and logged; existing config files are never corrupted
- **`post_write()` hook**: Well-designed trait extension for Neovim's secondary `.nvim-dap.lua` file
- **Standalone CLI mode**: `--dap-config vscode --dap-port 4711` works correctly for CI/scripting use cases

---

## Verification Checklist

- [ ] `cargo fmt --all` — Pass
- [ ] `cargo check --workspace` — Pass
- [ ] `cargo test --workspace` — Pass
- [ ] `cargo clippy --workspace -- -D warnings` — Pass
- [ ] `--dap-config` combined mode override wired through
- [ ] `clean_jsonc` duplication resolved
- [ ] Emacs merge path produces absolute paths
- [ ] Content comparison before file write
- [ ] `indoc` no-op removed

---

## Overall Assessment

The feature is architecturally well-designed and follows project patterns correctly. The generators are cleanly implemented with good test coverage. However, the `--dap-config` combined-mode override is a documented-as-working feature that silently does nothing — this must be fixed or honestly documented before merge. The code duplication and unconditional file writes are maintainability concerns that should be addressed in a follow-up.

**Recommendation:** Fix issue #1 (critical) and issues #2-5 (major), then re-review.
