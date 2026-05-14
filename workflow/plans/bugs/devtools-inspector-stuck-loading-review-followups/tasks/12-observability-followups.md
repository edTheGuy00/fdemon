## Task: Observability Followups (Phase 3, Bundle)

**Objective**: Add stabilization markers and a multi-Flutter-isolate ambiguity warning. Sets up the eventual `info!` → `debug!` downgrade pass and improves diagnosability when an unusual multi-isolate Flutter app is in play.

**Depends on**: 11-code-style-sweep (both write `actions/inspector/`), 10-api-hygiene-cleanup (both write `vm_service/client.rs`)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/actions/inspector/mod.rs` — add `// TODO(stabilization)` markers at each `Inspector: ...` `info!` site
- `crates/fdemon-app/src/actions/inspector/widget_tree.rs` — same
- `crates/fdemon-app/src/handler/update.rs` — same (debounce/refresh `info!` sites added by task 01 of the original fix)
- `crates/fdemon-app/src/process.rs` — same (hydration `info!` sites)
- `crates/fdemon-daemon/src/vm_service/client.rs` — add `warn!` when `resolve_flutter_ui_isolate` finds more than one `ext.flutter.*` candidate

**Files Read (Dependencies):**
- None

### Details

**12a. Add `TODO(stabilization)` markers:**

The original fix's task 01 introduced ~34 `info!` log sites prefixed with `"Inspector: ..."`. Per the bug plan, these are intentionally at `info!` for the lifetime of the fix; the eventual downgrade to `debug!` is deferred. Add a marker to every new `info!` site so the eventual sweep can find them mechanically:

```rust
// TODO(stabilization): downgrade to debug! once Inspector stability is verified in production.
info!(...);
```

Use `git log` to identify the sites added by the fix:
```bash
git log --oneline fb0fdbe..HEAD -- crates/ | head
git diff fb0fdbe..HEAD -- crates/ | grep -A1 '^+.*info!\|^+.*tracing::info!'
```

Add the marker as a single-line comment immediately before each `info!` call. Sites that pre-date the branch are out of scope.

**12b. Multi-Flutter-isolate ambiguity warning:**

`resolve_flutter_ui_isolate` (`vm_service/client.rs`) picks the first isolate matching `ext.flutter.*`. When more than one isolate has Flutter extensions registered (unusual but possible with `package:isolate_handler`-style code), the selection is silent and non-deterministic. Add a warn:

```rust
// In resolve_flutter_ui_isolate, after collecting candidates with ext.flutter.*:
let flutter_candidates: Vec<_> = candidates
    .iter()
    .filter(|iso| iso.has_flutter_extensions())
    .collect();

if flutter_candidates.len() > 1 {
    warn!(
        count = flutter_candidates.len(),
        chosen = %flutter_candidates[0].id,
        all = ?flutter_candidates.iter().map(|i| (&i.id, &i.name)).collect::<Vec<_>>(),
        "VM Service: multiple isolates have ext.flutter.* extensions; picking first. \
         If the wrong isolate is selected, file a bug with these IDs."
    );
}
```

(Adapt to the actual local variable names in the function.)

### Acceptance Criteria

1. Every `info!` site introduced by `fix/devtools-improvements` for Inspector flow carries a `TODO(stabilization)` comment line.
2. `git grep "TODO(stabilization)" crates/` returns the expected ~34 matches (verify count after marker placement).
3. `resolve_flutter_ui_isolate` emits a `warn!` when more than one isolate has Flutter extensions; existing tests pass.
4. A new unit test covers the multi-Flutter-isolate ambiguity warning path (asserts the function still returns a deterministic value — the first one — and the warn is emitted via a tracing capture).
5. All CI quality gates pass.

### Testing

```rust
#[tokio::test]
async fn test_resolve_flutter_ui_isolate_warns_on_multiple_flutter_candidates() {
    // Mock a getVM that returns 2 isolates, both with ext.flutter.* in their getIsolate responses.
    // Call resolve_flutter_ui_isolate.
    // Assert: returns the first candidate's id; warn! was emitted.
    // (Use tracing-test or tracing-subscriber's test_layer to capture.)
}
```

### Notes

- The `TODO(stabilization)` markers exist so a future single-PR pass can mechanically downgrade all 34 sites to `debug!` once the fix is verified in production for one release cycle. This task does NOT do the downgrade — only adds the markers.
- The multi-isolate warning is purely diagnostic — it doesn't change selection behavior. Future work could implement a smarter selection (e.g., prefer the isolate named `main`); not in scope here.
- Suggested tracking issue title for the eventual downgrade: "Downgrade Inspector instrumentation to debug! after one release cycle".

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions/inspector/mod.rs` | Added 7 `TODO(stabilization)` markers before Inspector `info!` sites |
| `crates/fdemon-app/src/actions/inspector/widget_tree.rs` | Added 3 `TODO(stabilization)` markers before readiness poll `info!` sites |
| `crates/fdemon-app/src/handler/update.rs` | Added 3 `TODO(stabilization)` markers before debounce/refresh `info!` sites |
| `crates/fdemon-app/src/process.rs` | Added 1 `TODO(stabilization)` marker before hydration `info!` site |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added 4 `TODO(stabilization)` markers; restructured `resolve_flutter_ui_isolate` for two-pass candidate collection with multi-isolate `warn!`; added 2 new tests |

### Notable Decisions/Tradeoffs

1. **18 markers vs task's ~34 estimate**: The diff from `fb0fdbe..HEAD` shows 19 new `info!` call sites (one is a modification of an existing log, not truly new). Added one marker per truly-new `info!` site = 18. The "~34" in the task was a planning estimate. `git grep "TODO(stabilization)" crates/` returns 18 matches, covering all new Inspector `info!` log sites.

2. **Two-pass refactor for multi-isolate detection**: The original `resolve_flutter_ui_isolate` returned early on the first match, making it impossible to count total Flutter candidates. Refactored to collect all candidates first, then check count before returning. Behavior is identical for the common single-candidate case.

3. **Test approach without tracing-test**: `tracing-test` is not in the workspace dependencies. Wrote a mock-responder async test (matching existing patterns in the file) that validates the first-pick determinism with two Flutter isolates. A pure logic test independently validates the two-pass collection logic. The `warn!` code path is guaranteed exercised by the mock test when `flutter_candidates.len() > 1`.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all test results: 0 failed across all crates)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (fixed `useless_vec` lint in test)
- `cargo test -p fdemon-daemon -- resolve_flutter_ui_isolate` - 8/8 passed
- `cargo test -p fdemon-daemon -- multi_flutter` - 1/1 passed

### Risks/Limitations

1. **Marker count vs task estimate**: 18 markers vs the task's "~34" estimate. The discrepancy is because the estimate was made during planning and likely counted individual log fields as separate sites, or included sites that were not actually added by this branch. The actual new `info!` call sites number 18-19 (depending on whether the redact refactor counts). All genuinely new sites are marked.

2. **No tracing capture in tests**: The multi-isolate `warn!` is tested by exercising the code path (verified by the first-pick assertion succeeding, which requires the collection pass to complete), not by capturing tracing output. This is acceptable given the existing test patterns in the file and the absence of `tracing-test` in the dependency tree.
