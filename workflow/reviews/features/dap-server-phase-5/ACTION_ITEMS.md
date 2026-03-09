# Action Items: DAP Server Phase 5 — IDE DAP Auto-Configuration

**Review Date:** 2026-03-09
**Verdict:** NEEDS WORK
**Blocking Issues:** 1 critical, 4 major

## Critical Issues (Must Fix)

### 1. `--dap-config` CLI override not threaded through in combined mode
- **Source:** All 4 review agents (architecture, quality, logic, risks)
- **Files:** `src/main.rs:130-138`, `crates/fdemon-app/src/handler/dap.rs:74-76`
- **Problem:** When `--dap-config <IDE>` is used without `--dap-port`, the parsed `ParentIde` is validated then discarded. The comment claims the override is "threaded through the action" but `handle_started()` always passes `ide_override: None`. The feature silently does nothing in combined mode.
- **Required Action:**
  1. Add `cli_dap_config_override: Option<ParentIde>` to `Engine` or `AppState`
  2. Populate it from `args.dap_config` during initialization (pass through runner functions)
  3. In `handle_started()`, read the stored override and pass it as `ide_override: Some(ide)`
  4. Fix the misleading comment at `main.rs:132-133`
  5. Add a test that sets the override and verifies `ide_override` is `Some` in the emitted action
- **Acceptance:** Running `fdemon --dap-config neovim` in a plain terminal (no `$NVIM` set) generates Neovim config when DAP starts

## Major Issues (Should Fix)

### 2. `clean_jsonc()` duplicated across two modules
- **Source:** Architecture, Code Quality, Risks agents
- **Files:** `crates/fdemon-app/src/ide_config/merge.rs:67-128`, `crates/fdemon-app/src/config/vscode.rs`
- **Problem:** ~120 lines of identical JSONC parsing code in two locations within the same crate
- **Suggested Action:** Extract `clean_jsonc`, `strip_json_comments`, `strip_trailing_commas` to a shared `pub(crate)` module (e.g., `crate::util::jsonc` or make them `pub(crate)` in `config/vscode.rs`). Delete the copies in `merge.rs` and import from the shared location.

### 3. Emacs `merge_config` produces relative placeholder path
- **Source:** Code Quality, Logic, Risks agents
- **File:** `crates/fdemon-app/src/ide_config/emacs.rs:57-59`
- **Problem:** When the Emacs config file already exists, the merge path produces `(load-file ".fdemon/dap-emacs.el")` instead of an absolute path. The `generate()` path correctly uses the absolute path.
- **Suggested Action:** In `run_generator()`, detect when the generator's merge is semantically an overwrite (e.g., Emacs) and call `generate(port, project_root)` instead of `merge_config()`. Alternatively, add `project_root` to the `merge_config` trait signature.

### 4. `run_generator` writes unconditionally without content comparison
- **Source:** Logic, Risks, Architecture agents
- **File:** `crates/fdemon-app/src/ide_config/mod.rs:138-150`
- **Problem:** Every DAP start overwrites config files even when content is identical. Causes unnecessary git diffs and makes `ConfigAction::Skipped` dead code.
- **Suggested Action:** Before writing, compare `content == existing`. If identical, return `ConfigAction::Skipped("content unchanged")`. This also gives the `Skipped` variant its first actual use.

### 5. `indoc()` no-op function with misleading doc comment
- **Source:** Code Quality agent
- **File:** `crates/fdemon-app/src/ide_config/helix.rs:162-164`
- **Problem:** Function claims to strip leading newlines but just calls `.to_string()`. Dead code with incorrect documentation.
- **Suggested Action:** Remove the function entirely. Replace `indoc(r#"..."#)` with `r#"..."#.to_string()` at the call site.

## Minor Issues (Consider Fixing)

### 6. Unsafe env var tests without thread isolation
- `crates/fdemon-app/src/config/settings.rs:1835-1858`
- Add `#[serial]` annotation or refactor to inject env reading

### 7. Public re-exports of internal merge utilities
- `crates/fdemon-app/src/ide_config/mod.rs:254-257`
- Change to `pub(crate)` — these are not public API

### 8. `unreachable!()` in helix merge
- `crates/fdemon-app/src/ide_config/helix.rs:207`
- Replace with `Error::config(...)` to avoid panics in library code

### 9. Redundant `is_some()` + branch pattern
- `crates/fdemon-app/src/actions/mod.rs:604-605`
- Replace with `ide_override.or_else(|| detect_parent_ide())`

### 10. Zed "Delve" adapter workaround undocumented in generated file
- `crates/fdemon-app/src/ide_config/zed.rs:44`
- Add a comment in the generated debug.json explaining the workaround

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All critical issues resolved (#1)
- [ ] All major issues resolved or justified (#2-#5)
- [ ] `cargo fmt --all` — Pass
- [ ] `cargo check --workspace` — Pass
- [ ] `cargo test --workspace` — Pass
- [ ] `cargo clippy --workspace -- -D warnings` — Pass
- [ ] Manual test: `fdemon --dap-config neovim` generates Neovim config on DAP start
- [ ] Manual test: `fdemon --dap-config vscode --dap-port 4711` generates and exits (standalone)
