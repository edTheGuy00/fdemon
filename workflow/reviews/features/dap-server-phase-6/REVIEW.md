# Review: DAP Server Phase 6 — Tier-1 Feature Completion & Variable System Overhaul

**Review Date:** 2026-03-20
**Branch:** `feat/dap-phase-6-plan` vs `main`
**Commits:** 19 (645967d → 1b83646)
**Scope:** 59 files changed, +15,934 / -196 lines (~4,700 production, ~9,300 tests)
**Tasks:** 18/18 completed

---

## Verdict: **NEEDS WORK**

Multiple agents returned CONCERNS; one returned NEEDS WORK. No blocking/critical architectural issues, but several correctness, quality, and security findings warrant attention before merge.

| Agent | Verdict | Critical | High/Major | Medium/Minor | Low/Nitpick |
|-------|---------|----------|------------|--------------|-------------|
| Architecture Enforcer | PASS | 0 | 0 | 2 warnings | 1 suggestion |
| Code Quality Inspector | NEEDS WORK | 1 | 8 | 8 | 3 |
| Logic & Reasoning | CONCERNS | 0 | 6 warnings | — | 5 notes |
| Risks & Trade-offs | CONCERNS | 0 | 2 medium | 5 low | — |
| Security Reviewer | CONCERNS | 0 | 2 high | 4 medium | 3 low |

---

## Consolidated Findings

Findings are deduplicated and merged where multiple agents flagged the same code. Severity uses: CRITICAL > HIGH > MEDIUM > LOW > INFO.

---

### HIGH — H1. `.expect()` panic in production request handler

**File:** `crates/fdemon-dap/src/adapter/handlers.rs:343`
**Source:** code_quality_inspector, risks_tradeoffs_analyzer, security_reviewer

```rust
self.breakpoint_state.lookup_by_dap_id(dap_id).expect("entry was just inserted");
```

A failed invariant (bug in `BreakpointState`, future refactoring) will panic the entire DAP session task with no recovery. This violates the project's error-handling standards. Same issue at `stack.rs:888` (`.expect("SDK sources must have a source reference")`).

**Required Action:** Replace with `.ok_or_else(|| DapResponse::error(...))` or graceful fallback.

---

### HIGH — H2. Expression injection in hover evaluate via `toString()` wrapping

**File:** `crates/fdemon-dap/src/adapter/evaluate.rs:278`
**Source:** security_reviewer

```rust
format!("({}).toString()", args.expression)
```

Client-supplied expression is embedded directly into a new Dart expression. Crafted input like `a) + sideEffect(` produces executable code. Hover evaluation is expected to be read-only.

**Required Action:** Call `toString()` on the result *object reference ID* returned by the initial evaluation (same pattern `enrich_with_to_string` uses in `variables.rs`), rather than re-composing a string containing the user's expression.

---

### HIGH — H3. `handle_restart` inconsistent with `handle_hot_restart`

**File:** `crates/fdemon-dap/src/adapter/handlers.rs:1686-1699`
**Source:** architecture_enforcer, logic_reasoning_checker

The standard DAP `restart` handler calls `hot_restart()` but does NOT:
- Emit `progressStart`/`progressEnd` events
- Emit `dart.hotRestartComplete` custom event
- Call `on_hot_restart()` to invalidate source refs and breakpoint state

Users using the standard IDE restart button get a worse experience than the custom `hotRestart` request.

**Required Action:** Delegate `handle_restart` to `handle_hot_restart` (or extract shared helper).

---

### HIGH — H4. `SourceReferenceStore::get_or_create` is O(n) linear scan

**File:** `crates/fdemon-dap/src/adapter/stack.rs:89-95`
**Source:** code_quality_inspector, risks_tradeoffs_analyzer

```rust
for (&id, entry) in &self.references {
    if entry.script_id == script_id && entry.isolate_id == isolate_id {
        return id;
    }
}
```

Called for every script in `loadedSources` and every frame in `stackTrace`. O(n^2) for apps with hundreds of scripts.

**Required Action:** Add a reverse-index `HashMap<(String, String), i64>` keyed by `(isolate_id, script_id)`.

---

### MEDIUM — M1. No authentication on DAP server

**File:** `crates/fdemon-dap/src/server/mod.rs:243`
**Source:** security_reviewer

Any local process can connect, evaluate arbitrary expressions, read application state, and forward VM Service RPCs via `callService`. The server is localhost-only by default, but this permits SSRF and multi-user attacks.

**Suggested Action:** Consider a startup-generated auth token required in `initialize` arguments. Document the open port and its capabilities in user-facing output.

---

### MEDIUM — M2. Sequential toString()/getter evaluation compounds latency

**Files:** `crates/fdemon-dap/src/adapter/variables.rs:537` (toString), `:1442` (getters)
**Source:** code_quality_inspector, risks_tradeoffs_analyzer

Sequential evaluation with 1s per-call timeout means 20 PlainInstance vars = up to 20s blocked, 50 getters = up to 50s. The IDE variables panel appears to hang.

**Suggested Action:** Add a global time budget (3-5s) for the entire toString enrichment loop and getter evaluation loop. When exhausted, skip remaining candidates.

---

### MEDIUM — M3. `callService` forwards arbitrary VM Service RPCs without allowlist

**File:** `crates/fdemon-dap/src/adapter/handlers.rs:1126`
**Source:** security_reviewer, risks_tradeoffs_analyzer

Any method + params forwarded verbatim. The code explicitly acknowledges this is by design ("the VM Service itself handles authorization"), and the localhost-only binding mitigates practical risk. However, it creates an unconstrained RPC proxy.

**Suggested Action:** Either document the full-passthrough design decision in a security policy, or implement a prefix-based allowlist (e.g., `ext.*` + known-good methods).

---

### MEDIUM — M4. Variable store cap provides no IDE feedback

**File:** `crates/fdemon-dap/src/adapter/types.rs` (MAX_VARIABLE_REFS = 10,000)
**Source:** risks_tradeoffs_analyzer

When the cap is hit, variables silently become non-expandable. The warning is logged but invisible to the user.

**Suggested Action:** Emit a DAP `output` event with category `"console"` the first time the cap is reached.

---

### MEDIUM — M5. Column=0 underflow in completions handler

**File:** `crates/fdemon-dap/src/adapter/handlers.rs:1613`
**Source:** security_reviewer, logic_reasoning_checker

```rust
let prefix_len = ((column - 1) as usize).min(text.len());
```

`column: 0` yields `-1i64`, which casts to `usize::MAX`. The `.min()` prevents a crash but produces wrong results (entire text used as prefix instead of empty).

**Required Action:** Guard `column < 1` with an early return or clamp.

---

### MEDIUM — M6. `evaluateName` does not escape special characters in map string keys

**File:** `crates/fdemon-dap/src/adapter/variables.rs:1313`
**Source:** logic_reasoning_checker

```rust
"String" => format!("{}[\"{}\"]", p, key_str),
```

Keys containing `"`, `\`, `$`, or `\n` produce invalid Dart expressions. "Add to Watch" and "Copy Expression" would break.

**Suggested Action:** Apply Dart string escaping on `key_str` before interpolation.

---

### MEDIUM — M7. Silent `unwrap_or_default()` swallows malformed attach arguments

**File:** `crates/fdemon-dap/src/adapter/handlers.rs:84`
**Source:** code_quality_inspector

```rust
serde_json::from_value(v.clone()).unwrap_or_default()
```

Malformed `attach` args silently fall back to defaults. User gets no indication their debug config is ignored. Inconsistent with all other handlers.

**Suggested Action:** Return a parse error response like other handlers do.

---

### MEDIUM — M8. Silently ignored `resume` errors in event handlers

**File:** `crates/fdemon-dap/src/adapter/events.rs:139,160,207`
**Source:** code_quality_inspector

```rust
let _ = self.backend.resume(&isolate_id, None, None).await;
```

If resume fails (isolate exited, connection dropped), the isolate stays paused indefinitely with no indication.

**Suggested Action:** Log at `warn!` level instead of silently discarding.

---

### LOW — L1. `exception_refs` not cleared in `on_resume()`

**File:** `crates/fdemon-dap/src/adapter/events.rs:582`
**Source:** risks_tradeoffs_analyzer, logic_reasoning_checker

`on_resume()` clears `var_store`, `frame_store`, `evaluate_name_map` but not `exception_refs`. Brief inconsistency window between eager resume and deferred `Resumed` event.

**Suggested Action:** Add `self.exception_refs.clear()` to `on_resume()`.

---

### LOW — L2. `exception_refs` is `pub` instead of `pub(crate)`

**File:** `crates/fdemon-dap/src/adapter/mod.rs:173`
**Source:** code_quality_inspector, logic_reasoning_checker

Unnecessarily exposes internal state across crate boundaries.

**Suggested Action:** Restrict to `pub(crate)`.

---

### LOW — L3. Dead `#[allow(dead_code)]` error constants

**File:** `crates/fdemon-dap/src/adapter/types.rs:256-269`
**Source:** code_quality_inspector, risks_tradeoffs_analyzer

4 of 5 error constants unused, silenced with `#[allow(dead_code)]`. Either wire them into error responses or remove them.

---

### LOW — L4. Pointless self-assignment in variables.rs

**File:** `crates/fdemon-dap/src/adapter/variables.rs:714`
**Source:** code_quality_inspector

```rust
var.name = var.name.clone(); // ensure owned
```

Does nothing — `var.name` is already an owned `String`. Remove.

---

### LOW — L5. Unnecessary `.clone()` on destructured owned values

**Files:** `crates/fdemon-dap/src/adapter/variables.rs:362,377`
**Source:** code_quality_inspector

`&isolate_id.clone()` where `&isolate_id` suffices (values are already owned from match destructuring).

---

### LOW — L6. `VariableRef::Scope` lacks `isolate_id` — ambiguous in multi-isolate

**File:** `crates/fdemon-dap/src/adapter/stack.rs:171-178`
**Source:** logic_reasoning_checker

`lookup_by_index` returns the first frame matching the index across ALL isolates. Structurally fragile for multi-isolate debugging (though mitigated by Flutter's typical single-isolate model).

---

### LOW — L7. `get_source` uses `String` error type instead of `BackendError`

**File:** `crates/fdemon-dap/src/adapter/backend.rs`
**Source:** code_quality_inspector

Inconsistent with all other backend trait methods.

---

### LOW — L8. Hot-reload/hot-restart handler duplication

**File:** `crates/fdemon-dap/src/adapter/handlers.rs`
**Source:** code_quality_inspector

~50 lines of identical structure differing only in title string, backend method, and event name. Should be extracted into a shared helper.

---

### LOW — L9. No idle timeout after initialization

**File:** `crates/fdemon-dap/src/server/session.rs:54`
**Source:** security_reviewer

A client that sends `initialize` then goes silent holds a semaphore slot indefinitely. With 8 slots, this enables trivial local DoS.

---

### LOW — L10. Stale module-level docs in `mod.rs`

**File:** `crates/fdemon-dap/src/adapter/mod.rs`
**Source:** code_quality_inspector

Module doc still references the old four-module list; Phase 6 added `handlers.rs`, `events.rs`, `variables.rs`.

---

### LOW — L11. `get_source_report` has duplicated JSON param construction

**File:** `crates/fdemon-app/src/handler/dap_backend.rs`
**Source:** architecture_enforcer

Trait method and boxed vtable both independently construct the same JSON params. Extract a `build_source_report_params` helper.

---

### INFO — Typed data list variants incomplete

**File:** `crates/fdemon-dap/src/adapter/variables.rs:907`
**Source:** code_quality_inspector

Only `Uint8List`, `Int32List`, `Float64List` handled. Missing: `Int8List`, `Uint16List`, `Int16List`, etc. Falls through to "PlainInstance" rendering. A comment documenting intentional partial coverage would help.

---

### INFO — `DapExceptionPauseMode::None` shadows prelude `None`

**File:** `crates/fdemon-dap/src/adapter/types.rs`
**Source:** code_quality_inspector

Creates unnecessary cognitive load in match arms.

---

### INFO — `on_resume()` called before backend resume succeeds

**Source:** logic_reasoning_checker

If `backend.resume()` fails, stores are already cleared but isolate is still paused. Follows Dart DDS adapter behavior (eager invalidation). Documented trade-off.

---

## Documentation Freshness Check

| Document | Update Needed? | Reason |
|----------|---------------|--------|
| `docs/ARCHITECTURE.md` | Already updated | Task 18 updated it for Phase 6 |
| Module docs in `adapter/mod.rs` | YES | Stale submodule list (L10) |

---

## Technical Debt Introduced

| Item | Severity | Cost to Fix |
|------|----------|-------------|
| `variables.rs` at ~1,596 lines | Low | Extract `globals.rs`, `type_rendering.rs` |
| `handlers.rs` at ~1,925 lines | Low | Extract `completions.rs`, `debug_options.rs` |
| Hot-reload/restart handler duplication | Low | Extract parameterized helper |
| `evaluate_name_map` has no capacity cap | Low | Add check at insert sites |
| Unused error code constants with `#[allow(dead_code)]` | Low | Wire into responses or remove |

---

## Test Coverage Assessment

- **New tests:** ~9,300 lines across 13 test files
- **Coverage areas:** All 18 tasks have dedicated test modules
- **Edge cases tested:** Timeouts, empty responses, malformed data, async frame rejection, progress pairing
- **Rating:** 5/5 — Extensive and well-structured

---

## Summary

### Strengths
- Clean architectural boundaries — `fdemon-dap` has zero imports from `fdemon-app`/`fdemon-daemon`
- All `DebugBackend` trait methods consistently defined, delegated, and noop-implemented
- Comprehensive timeout handling via centralized `with_timeout` wrapper
- Progress events properly paired in success and failure paths
- Extensive test coverage (~9,300 lines of new tests)
- Good use of named constants with derivation comments

### Required Before Merge (4 items)
1. **H1:** Replace `.expect()` panics with proper error handling
2. **H2:** Fix hover expression injection in `evaluate.rs`
3. **H3:** Align `handle_restart` with `handle_hot_restart`
4. **M5:** Guard `column=0` in completions handler

### Recommended Before Merge (4 items)
5. **H4:** Add reverse-index to `SourceReferenceStore`
6. **M6:** Escape special chars in evaluateName map keys
7. **M7:** Return error on malformed attach arguments
8. **M8:** Log silently-ignored resume errors

### Track as Follow-ups (remaining items)
- M1 (auth token), M2 (time budgets), M3 (callService allowlist), M4 (cap feedback)
- All LOW and INFO items
- File size technical debt
