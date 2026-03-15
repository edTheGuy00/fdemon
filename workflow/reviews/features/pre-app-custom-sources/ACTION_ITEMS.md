# Action Items: Pre-App Custom Sources (Phase 1)

**Review Date:** 2026-03-15
**Verdict:** NEEDS WORK
**Blocking Issues:** 3

## Critical Issues (Must Fix)

### 1. Auto-launch path bypasses pre-app source gating
- **Source:** Logic & Reasoning Checker
- **File:** `crates/fdemon-app/src/handler/update.rs` (~line 903, `AutoLaunchResult` handler)
- **Problem:** The `AutoLaunchResult` message handler returns `SpawnSession` directly without checking for pre-app sources. Users with `auto_start = true` behavior AND `start_before_app = true` custom sources will launch Flutter without waiting for dependencies.
- **Required Action:** Apply the same gate as `handle_launch()`: check `state.settings.native_logs.enabled && state.settings.native_logs.has_pre_app_sources()` and return `SpawnPreAppSources` instead of `SpawnSession` when pre-app sources exist. Add a test that verifies auto-launch respects pre-app gating.
- **Acceptance:** A test asserts that `AutoLaunchResult` returns `SpawnPreAppSources` when pre-app sources are configured.

## Major Issues (Should Fix)

### 2. HTTP health check buffer too small for reliable operation
- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/ready_check.rs:129-155`
- **Problem:** Single `read()` with 256-byte buffer. TCP may deliver partial data, misclassifying healthy servers as unhealthy.
- **Suggested Action:** Replace raw `read()` with `BufReader::read_line()` to read the complete HTTP status line reliably.
- **Acceptance:** `try_http_get()` correctly reads the full status line regardless of TCP segmentation.

### 3. `pub mod ready_check` should be `pub(super) mod ready_check`
- **Source:** Architecture Enforcer, Code Quality Inspector
- **File:** `crates/fdemon-app/src/actions/mod.rs:25`
- **Problem:** Breaks the `pub(super)` convention used by all sibling modules in `actions/`.
- **Suggested Action:** Change to `pub(super) mod ready_check;`.
- **Acceptance:** Module compiles with `pub(super)` and all tests pass.

## Minor Issues (Consider Fixing)

### 4. Decompose `spawn_pre_app_sources` (~237 lines)
- Extract per-source spawn logic into a helper to meet the 50-line function guideline.

### 5. `describe_ready_check` → `Display` impl on `ReadyCheck`
- Move the description logic to `impl Display for ReadyCheck` in `config/types.rs`.

### 6. Align `run_command_check` timeout pattern
- Use `remaining.is_zero()` after `saturating_sub` instead of `start.elapsed() >= timeout`.

### 7. Resolve `url` crate vs `parse_http_url` inconsistency
- Use the same parser for validation and runtime, or remove the `url` dependency.

### 8. Fix fragile TCP timeout test
- Replace port 1 with a dynamically bound-then-dropped port for deterministic behavior.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All critical issues resolved
- [ ] All major issues resolved or justified
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings`
