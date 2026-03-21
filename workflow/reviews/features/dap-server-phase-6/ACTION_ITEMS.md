# Action Items: DAP Server Phase 6

**Review Date:** 2026-03-20
**Verdict:** NEEDS WORK
**Blocking Issues:** 4 (must fix before merge)

---

## Critical Issues (Must Fix)

### 1. Replace `.expect()` panics with error handling [H1]
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer, security_reviewer
- **File:** `crates/fdemon-dap/src/adapter/handlers.rs:343`
- **File:** `crates/fdemon-dap/src/adapter/stack.rs:888`
- **Problem:** `.expect()` in production request handlers will panic the session task on invariant violation with no recovery path
- **Required Action:** Replace with `.ok_or_else(|| DapResponse::error(...))` or graceful fallback. At `stack.rs:888`, use `unwrap_or(0)` so missing source refs produce a DapSource with no reference rather than crashing.
- **Acceptance:** No `.expect()` or `.unwrap()` in non-test code paths without `// SAFETY:` justification

### 2. Fix hover expression injection [H2]
- **Source:** security_reviewer
- **File:** `crates/fdemon-dap/src/adapter/evaluate.rs:278`
- **Problem:** `format!("({}).toString()", args.expression)` embeds client input into executable Dart code
- **Required Action:** Evaluate `toString()` on the result object ID from the initial evaluation (call `backend.evaluate(isolate_id, &result_ref_id, "toString()")`) rather than re-composing an expression string containing user input
- **Acceptance:** Hover evaluate never constructs a Dart expression containing the raw user-supplied expression string

### 3. Align `handle_restart` with `handle_hot_restart` [H3]
- **Source:** architecture_enforcer, logic_reasoning_checker
- **File:** `crates/fdemon-dap/src/adapter/handlers.rs:1686-1699`
- **Problem:** Standard DAP `restart` handler misses progress events, `dart.hotRestartComplete`, and `on_hot_restart()` state invalidation
- **Required Action:** Either delegate to `handle_hot_restart` or extract a shared `execute_hot_restart_with_progress` helper called by both
- **Acceptance:** `restart` and `hotRestart` requests produce identical progress events, custom events, and state invalidation

### 4. Guard column=0 in completions handler [M5]
- **Source:** security_reviewer, logic_reasoning_checker
- **File:** `crates/fdemon-dap/src/adapter/handlers.rs:1613`
- **Problem:** `column: 0` causes `-1i64 as usize` = `usize::MAX`, producing incorrect prefix matching
- **Required Action:** Add `if column < 1 { return DapResponse::error(request, "column must be >= 1") }` before the subtraction
- **Acceptance:** Sending `column: 0` returns an error response (or clamps to 1), not wrong results

---

## Major Issues (Should Fix)

### 5. Add reverse-index to `SourceReferenceStore` [H4]
- **Source:** code_quality_inspector, risks_tradeoffs_analyzer
- **File:** `crates/fdemon-dap/src/adapter/stack.rs:89-95`
- **Problem:** O(n) linear scan per call, O(n^2) for `loadedSources` with many scripts
- **Suggested Action:** Add `HashMap<(String, String), i64>` keyed by `(isolate_id, script_id)` as reverse index
- **Acceptance:** `get_or_create` is O(1) lookup

### 6. Escape special characters in evaluateName map keys [M6]
- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-dap/src/adapter/variables.rs:1313`
- **Problem:** `format!("{}[\"{}\"]", p, key_str)` produces invalid Dart for keys with `"`, `\`, `$`, `\n`
- **Suggested Action:** Apply Dart string escaping to `key_str` before interpolation (at minimum: `\` → `\\`, `"` → `\"`, `$` → `\$`, newline → `\n`)
- **Acceptance:** `evaluateName` for map entries with special-char keys produces valid Dart expressions

### 7. Return error on malformed attach arguments [M7]
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-dap/src/adapter/handlers.rs:84`
- **Problem:** `unwrap_or_default()` silently ignores malformed attach args
- **Suggested Action:** Match other handlers' pattern — return a parse error to the IDE
- **Acceptance:** Malformed `attach` arguments produce an error response, not silent defaults

### 8. Log silently-ignored resume errors [M8]
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-dap/src/adapter/events.rs:139,160,207`
- **Problem:** `let _ = self.backend.resume(...)` discards errors silently; isolate could stay paused
- **Suggested Action:** Replace with `if let Err(e) = ... { warn!("Failed to auto-resume isolate: {}", e); }`
- **Acceptance:** Failed auto-resume is logged at warn level

---

## Minor Issues (Consider Fixing)

### 9. Clear `exception_refs` in `on_resume()` [L1]
- Add `self.exception_refs.clear()` for consistency with `var_store`/`frame_store`/`evaluate_name_map`

### 10. Restrict `exception_refs` to `pub(crate)` [L2]
- Change `pub exception_refs` to `pub(crate) exception_refs` in `adapter/mod.rs:173`

### 11. Remove dead `#[allow(dead_code)]` error constants [L3]
- Either wire `ERR_NOT_CONNECTED` etc. into error responses or delete them

### 12. Remove pointless self-assignment [L4]
- Delete `var.name = var.name.clone(); // ensure owned` at `variables.rs:714`

### 13. Remove unnecessary `.clone()` on owned values [L5]
- Replace `&isolate_id.clone()` with `&isolate_id` at `variables.rs:362,377`

### 14. Extract hot-reload/restart handler duplication [L8]
- Extract shared helper parameterized by operation name, backend call, event name

### 15. Update stale module docs [L10]
- Update module-level doc comment in `adapter/mod.rs` to include `handlers`, `events`, `variables`

---

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Items 1-4 (blocking) resolved
- [ ] Items 5-8 (major) resolved or justified
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean
- [ ] No `.expect()` or `.unwrap()` in non-test adapter code without `// SAFETY:` justification
- [ ] `grep -rn "\.expect(" crates/fdemon-dap/src/adapter/ | grep -v test | grep -v "// SAFETY"` returns zero results
