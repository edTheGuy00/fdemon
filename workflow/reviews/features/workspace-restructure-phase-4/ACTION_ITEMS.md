# Action Items: Workspace Restructure Phase 4

**Review Date:** 2026-02-08
**Verdict:** APPROVED WITH CONCERNS
**Blocking Issues:** 2

## Critical Issues (Must Fix)

### 1. Remove `devices_stub` module and dead code chain in startup.rs
- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-tui/src/startup.rs:28-47, 68-326`
- **Problem:** `unimplemented!()` stubs that panic at runtime. 260+ lines of dead code with `TODO(phase-4)` comments not addressed.
- **Required Action:** Remove `devices_stub` module and all functions marked with `TODO(phase-4): Remove after cleanup` (`animate_during_async`, `auto_start_session`, `try_auto_start_config`, `launch_with_validated_selection`, `launch_session`, `enter_normal_mode_disconnected`, `cleanup_sessions`, `StartupAction::AutoStart`)
- **Acceptance:** No `unimplemented!()` calls in codebase. `cargo check -p fdemon-tui` passes.

### 2. Restrict or document `dispatch_action()` limitations
- **Source:** Logic & Reasoning Checker
- **File:** `crates/fdemon-app/src/engine.rs:330-341`
- **Problem:** Public method silently fails for most `UpdateAction` variants due to hardcoded `None`/default parameters.
- **Required Action:** Either:
  - (a) Change to `pub(crate)` (only headless runner uses it), OR
  - (b) Add doc comment listing supported actions (currently only `SpawnSession`), OR
  - (c) Accept `ToolAvailability` as a parameter
- **Acceptance:** Method has clear documentation about which actions work, or is `pub(crate)`.

## Major Issues (Should Fix)

### 1. Replace blanket `#[allow(dead_code)]` on handler submodules
- **Source:** Architecture Enforcer, Code Quality Inspector, Logic & Reasoning Checker
- **File:** `crates/fdemon-app/src/handler/mod.rs:18-37`
- **Problem:** Masks genuinely dead code across 10+ submodules.
- **Suggested Action:** Remove blanket suppressions, identify specific dead items, apply targeted `#[allow(dead_code)]` with justification.

### 2. Guard Message clone behind plugins check
- **Source:** Code Quality Inspector, Logic & Reasoning Checker
- **File:** `crates/fdemon-app/src/engine.rs:236`
- **Suggested Action:** `let msg_for_plugins = if self.plugins.is_empty() { None } else { Some(msg.clone()) };`

### 3. Remove unused `PACKAGE_PATH_REGEX`
- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-core/src/stack_trace.rs:41-44`
- **Suggested Action:** Delete the static entirely.

### 4. Fix clippy quality gate
- **Source:** Risks & Tradeoffs Analyzer
- **Suggested Action:** Move `has_flutter_dependency` to `#[cfg(test)]`, remove `PACKAGE_PATH_REGEX`. Verify `cargo clippy --workspace -- -D warnings` passes.

## Minor Issues (Consider Fixing)

### 1. Downgrade debug logging in event.rs
- `warn!("ENTER/SPACE KEY DETECTED")` -> `trace!` or remove

### 2. Document plugin callback ordering in trait docs
- `on_event` fires before `on_message` -- add to `EnginePlugin` rustdoc

### 3. Move `has_flutter_dependency` to `#[cfg(test)]` block
- Only used in tests, marked dead code in production

### 4. Add `pub use handler::update` to fdemon-app lib.rs
- E2E tests use `fdemon_app::handler::update` path

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All critical issues resolved
- [ ] All major issues resolved or justified
- [ ] `cargo fmt --all` -- formatted
- [ ] `cargo check --workspace` -- compiles
- [ ] `cargo test --workspace --lib` -- all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` -- clean (no warnings)
- [ ] No `unimplemented!()` calls in production code
