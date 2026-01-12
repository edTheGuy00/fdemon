# Action Items: Phase 4 - Native Device Discovery

**Review Date:** 2026-01-12
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 4

---

## Critical Issues (Must Fix)

None.

---

## Major Issues (Must Fix Before Merge)

### 1. Unused `_avd_name` Parameter
- **Source:** Code Quality Inspector
- **File:** `src/daemon/avds.rs:133`
- **Line:** 133
- **Problem:** The `is_avd_running(_avd_name: &str)` function accepts an AVD name parameter but doesn't actually use it. It only checks if *any* emulator is running via `adb devices`.
- **Required Action:** Either:
  - Implement proper AVD-specific checking (query emulator console), OR
  - Remove the parameter and rename to `is_any_emulator_running()` to match actual behavior
- **Acceptance:** Function signature accurately reflects its behavior

### 2. Regex Compiled on Every Call
- **Source:** Code Quality Inspector
- **File:** `src/daemon/avds.rs:81-89`
- **Line:** 81
- **Problem:** `parse_avd_name()` creates a new Regex object on every invocation, which is inefficient for repeated calls.
- **Required Action:** Use static initialization:
```rust
use once_cell::sync::Lazy;
use regex::Regex;

static API_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"_API_(\d+)$").expect("Invalid API pattern regex")
});
```
- **Acceptance:** Regex compiled once per process, `cargo test avds` passes

### 3. Tool Availability Cache Not Used
- **Source:** Code Quality Inspector, Risks & Tradeoffs Analyzer
- **File:** `src/tui/spawn.rs`
- **Lines:** 283, 309
- **Problem:** `spawn_bootable_device_discovery()` and `spawn_device_boot()` call `ToolAvailability::check().await` fresh each time instead of using the cached `AppState.tool_availability`.
- **Required Action:**
  1. Modify spawn functions to accept `ToolAvailability` parameter
  2. Pass cached value from state through message/action
  3. Remove redundant `ToolAvailability::check()` calls
- **Acceptance:** No `ToolAvailability::check()` calls in spawn.rs except at app startup

### 4. Duplicate BootableDevice Types
- **Source:** Architecture Enforcer, Risks & Tradeoffs Analyzer
- **Files:**
  - `src/daemon/mod.rs:42` - `enum BootableDevice { IosSimulator(...), AndroidAvd(...) }`
  - `src/core/types.rs:667` - `struct BootableDevice { id, name, platform, runtime, state }`
- **Problem:** Two distinct types with the same name in different layers creates confusion and requires manual conversion code in handlers.
- **Required Action:** Choose one approach:
  - **Option A (Recommended):** Keep `core::BootableDevice` struct as the canonical type. Add `impl From<IosSimulator> for BootableDevice` and `impl From<AndroidAvd> for BootableDevice` traits. Remove the daemon enum.
  - **Option B:** Rename daemon type to `daemon::BootCommand` to clearly distinguish from `core::BootableDevice`.
  - **Option C:** Document the design decision and add explicit conversion helper functions.
- **Acceptance:** Clear separation or unification with documented rationale

---

## Minor Issues (Should Fix)

### 1. Magic Number: iOS Boot Timeout
- **File:** `src/daemon/simulators.rs:165`
- **Problem:** Hardcoded `Duration::from_secs(60)`
- **Suggested Action:**
```rust
const SIMULATOR_BOOT_TIMEOUT: Duration = Duration::from_secs(60);
```

### 2. Magic Number: Android Init Delay
- **File:** `src/daemon/avds.rs:125`
- **Problem:** Hardcoded `Duration::from_secs(2)`
- **Suggested Action:**
```rust
const AVD_INIT_DELAY: Duration = Duration::from_secs(2);
```

### 3. Swallowed Errors in Tool Checks
- **File:** `src/daemon/tool_availability.rs:51`
- **Problem:** `.unwrap_or(false)` hides command execution failures
- **Suggested Action:**
```rust
.inspect_err(|e| tracing::debug!("xcrun simctl check failed: {}", e))
.unwrap_or(false)
```

### 4. Platform String Matching
- **File:** `src/tui/spawn.rs:306`
- **Problem:** Uses `platform.as_str()` with string matching
- **Suggested Action:** Accept `core::Platform` enum instead of `String`

### 5. Large Enum in mod.rs
- **File:** `src/daemon/mod.rs`
- **Problem:** BootableDevice enum (~110 lines) in mod.rs
- **Suggested Action:** Move to `src/daemon/bootable_device.rs`

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Issue #1: `_avd_name` parameter removed or implemented
- [ ] Issue #2: Regex uses static initialization (check with `cargo test avds`)
- [ ] Issue #3: No `ToolAvailability::check()` in spawn.rs
- [ ] Issue #4: BootableDevice types unified or clearly separated
- [ ] `cargo fmt` - Code is formatted
- [ ] `cargo check` - No compilation errors
- [ ] `cargo test --lib` - All tests pass
- [ ] `cargo clippy -- -D warnings` - No clippy warnings

---

## Suggested Fix Order

1. **Issue #2** (Regex) - Quick fix, no API changes
2. **Issue #1** (Unused param) - Simple fix, clarifies API
3. **Issue #3** (Cache) - Requires message/action changes
4. **Issue #4** (Dual types) - Architectural decision needed

---

## Notes

- E2E tests were reported failing in completion summary but are pre-existing issues
- Unit test coverage is good (24 tests across new modules)
- No security concerns identified
- Implementation is functional, issues are quality/maintainability focused
