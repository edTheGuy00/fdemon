# Action Items: E2E Testing Phase 3 Wave 1

**Review Date:** 2026-01-08
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 3

---

## Critical Issues (Must Fix Before Wave 2)

### 1. Missing Drop Implementation for Process Cleanup

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `tests/e2e/pty_utils.rs`
- **Line:** After line 172 (end of `impl FdemonSession`)
- **Problem:** If a test panics before calling `quit()` or `kill()`, the spawned fdemon process continues running. Orphaned processes accumulate and can interfere with subsequent tests.
- **Required Action:** Implement the `Drop` trait for `FdemonSession` to ensure cleanup on panic or early return.

**Implementation:**
```rust
impl Drop for FdemonSession {
    fn drop(&mut self) {
        // Best-effort cleanup - ignore errors during drop
        let _ = self.kill();
    }
}
```

- **Acceptance:**
  - Add test that panics mid-session and verify no orphaned process
  - Run `pgrep fdemon` before and after test to confirm cleanup

---

### 2. Race Condition in quit() Method

- **Source:** Logic Reasoning Checker
- **File:** `tests/e2e/pty_utils.rs`
- **Line:** 131-146
- **Problem:** Uses hardcoded 500ms sleep that doesn't account for slow CI environments or actual process state. Returns success even if process might still be alive.

**Current Code:**
```rust
pub fn quit(&mut self) -> PtyResult<()> {
    self.send_key('q')?;
    std::thread::sleep(Duration::from_millis(500));  // Fixed delay
    let alive = self.session.is_alive()?;
    if alive {
        self.kill()?;
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())  // No verification!
}
```

- **Required Action:** Replace fixed sleep with retry loop that polls process state.

**Implementation:**
```rust
/// Graceful quit timeout - how long to wait before force-killing
const QUIT_TIMEOUT_MS: u64 = 2000;
const QUIT_POLL_INTERVAL_MS: u64 = 100;

pub fn quit(&mut self) -> PtyResult<()> {
    self.send_key('q')?;

    // Wait for graceful shutdown with polling
    let iterations = QUIT_TIMEOUT_MS / QUIT_POLL_INTERVAL_MS;
    for _ in 0..iterations {
        std::thread::sleep(Duration::from_millis(QUIT_POLL_INTERVAL_MS));
        if !self.session.is_alive()? {
            return Ok(());
        }
    }

    // Still alive after timeout, force kill
    self.kill()?;

    // Verify termination
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(QUIT_POLL_INTERVAL_MS));
        if !self.session.is_alive()? {
            return Ok(());
        }
    }

    Err("Process did not terminate after kill".into())
}
```

- **Acceptance:**
  - `quit()` returns error if process doesn't terminate
  - No hardcoded 500ms sleep
  - Process state verified before returning success

---

### 3. No Test Isolation Strategy

- **Source:** Risks & Tradeoffs Analyzer
- **File:** `tests/e2e/pty_utils.rs` (tests module) and future tests
- **Problem:** Multiple PTY tests running in parallel could spawn fdemon on same fixture simultaneously, causing interference and flaky tests.
- **Required Action:** Add serialization for PTY tests using `serial_test` crate or similar.

**Implementation Option A (Recommended):** Add `serial_test` dependency and mark PTY tests:

1. Add to `Cargo.toml`:
```toml
[dev-dependencies]
serial_test = "3"
```

2. Mark PTY tests:
```rust
use serial_test::serial;

#[test]
#[ignore]
#[serial]
fn test_spawn_fdemon() { ... }
```

**Implementation Option B:** Create fixture lock file:
```rust
fn acquire_fixture_lock(fixture_name: &str) -> std::fs::File {
    let lock_path = std::env::temp_dir().join(format!("fdemon_test_{}.lock", fixture_name));
    let file = std::fs::File::create(&lock_path).unwrap();
    file.try_lock_exclusive().unwrap();
    file
}
```

- **Acceptance:**
  - PTY tests can run with `cargo test -- --test-threads=4` without interference
  - No fixture conflicts when multiple tests run simultaneously

---

## Major Issues (Should Fix)

### 4. capture_screen() Logic May Not Work As Expected

- **Source:** Logic Reasoning Checker
- **File:** `tests/e2e/pty_utils.rs`
- **Line:** 118-127
- **Problem:** Uses `found.before()` which returns bytes BEFORE the match, likely empty. Also silently returns empty string on timeout.

**Current Code:**
```rust
pub fn capture_screen(&mut self) -> PtyResult<String> {
    match self.session.expect(Regex(".*")) {
        Ok(found) => {
            let bytes = found.before();  // Wrong - bytes BEFORE match
            Ok(String::from_utf8_lossy(bytes).to_string())
        }
        Err(_) => Ok(String::new()),  // Silent failure
    }
}
```

- **Suggested Action:** Investigate correct `expectrl` API for reading available output. Consider:
  1. Using `session.try_read()` if available
  2. Using `found.matches()` or full output
  3. Documenting current behavior if intentional

- **Acceptance:** Method actually returns terminal content, or behavior is clearly documented.

---

### 5. Missing Method Documentation

- **Source:** Code Quality Inspector
- **File:** `tests/e2e/pty_utils.rs`
- **Lines:** 24-172 (all public methods)
- **Problem:** Public methods lack `///` doc comments explaining parameters, return values, and usage.

- **Suggested Action:** Add doc comments to at minimum:
  - `spawn()` / `spawn_with_args()`
  - `expect_header()`, `expect_running()`, `expect_reloading()`
  - `send_key()`, `send_special()`
  - `capture_screen()`
  - `quit()`, `kill()`

**Example:**
```rust
/// Wait for fdemon to show the header with project name.
///
/// Matches against patterns "Flutter Demon" or "fdemon" in terminal output.
/// Uses default timeout of 10 seconds.
///
/// # Errors
/// Returns error if pattern is not found within timeout.
pub fn expect_header(&mut self) -> PtyResult<()> {
```

- **Acceptance:** All public methods have doc comments.

---

### 6. Hardcoded Magic Numbers for Timeouts

- **Source:** Code Quality Inspector
- **File:** `tests/e2e/pty_utils.rs`
- **Lines:** 136, 143, 153, 157 (various sleep calls)
- **Problem:** Magic numbers `500`, `100` used without named constants or documentation.

- **Suggested Action:** Extract to named constants:
```rust
/// Time to wait for graceful quit before force-killing
const QUIT_GRACE_PERIOD_MS: u64 = 500;

/// Time to wait between kill attempts
const KILL_RETRY_DELAY_MS: u64 = 100;
```

- **Acceptance:** No magic numbers in sleep() calls.

---

## Minor Issues (Consider Fixing)

### 7. Binary Path Resolution Could Be More Robust

- **Source:** Logic Reasoning Checker, Risks Analyzer
- **File:** `tests/e2e/pty_utils.rs`
- **Line:** 31-35
- **Problem:** Falls back to `target/debug/fdemon` without checking if it exists, and doesn't try release build.

- **Suggested Action:** Check multiple paths and provide helpful error:
```rust
let binary_path = std::env::var("CARGO_BIN_EXE_fdemon")
    .ok()
    .or_else(|| {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let debug = format!("{}/target/debug/fdemon", manifest);
        let release = format!("{}/target/release/fdemon", manifest);
        [release, debug].into_iter().find(|p| Path::new(p).exists())
    })
    .ok_or("fdemon binary not found. Run `cargo build` first.")?;
```

---

### 8. SpecialKey Could Derive More Traits

- **Source:** Code Quality Inspector
- **File:** `tests/e2e/pty_utils.rs`
- **Line:** 175
- **Problem:** Only derives `Debug, Clone, Copy` but `PartialEq, Eq` would improve testability.

- **Suggested Action:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialKey {
```

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] All critical issues resolved (Drop impl, quit() fix, test isolation)
- [ ] All major issues resolved or justified
- [ ] `cargo fmt` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes (all 1255+ tests)
- [ ] `cargo test --test e2e -- --ignored` passes (PTY tests)
- [ ] No orphaned fdemon processes after test run (`pgrep fdemon` returns empty)

---

## Priority Summary

| Priority | Issue | Effort |
|----------|-------|--------|
| 🔴 Critical | Implement Drop trait | Low (10 min) |
| 🔴 Critical | Fix quit() race condition | Medium (30 min) |
| 🔴 Critical | Add test isolation | Medium (30 min) |
| 🟠 Major | Fix/document capture_screen() | Medium (30 min) |
| 🟠 Major | Add method documentation | Medium (45 min) |
| 🟠 Major | Extract magic numbers | Low (15 min) |
| 🟡 Minor | Improve binary path resolution | Low (15 min) |
| 🔵 Nitpick | Add PartialEq to SpecialKey | Low (5 min) |

**Estimated Total Effort:** ~3 hours

---

**Next Steps:**
1. Create follow-up task for Phase 3 fixes
2. Address critical issues before starting Wave 2
3. Re-run review after fixes
