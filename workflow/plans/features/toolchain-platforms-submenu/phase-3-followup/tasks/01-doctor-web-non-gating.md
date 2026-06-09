## Task: [BLOCKING] `fdemon doctor` — exempt WebBrowser from exit-code gating

**Objective**: Make the `fdemon doctor` CLI honour the Phase-3 "Web never blocks" contract. Today a
`Missing` `WebBrowser` makes `fdemon doctor` exit `1` on any browser-less host (CI containers, headless
servers) because the gating loop treats every non-Android component as a hard gate. Exempt `WebBrowser`
from exit-code gating (it is still printed), extract the gating decision into a pure, unit-tested helper,
and update the module doc.

**Depends on**: Phase 3 (merged). Review finding A (MAJOR/blocking).

**Agent:** implementor

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `src/doctor.rs` — gating helper + WebBrowser exemption + module doc + tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `ComponentKind`, `ComponentStatus`.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name.

#### 1. Extract a pure gating helper

The gating decision is currently inlined in the `run_doctor` loop (`src/doctor.rs:94–103`):

```rust
let gates = if is_android_component(&c.kind) { android_gates } else { true };
if gates && is_failing(&c.status) { all_ok = false; }
```

Extract the decision into a pure, testable helper alongside `is_android_component` / `is_failing`
(`:31–46`):

```rust
/// Return `true` when a component contributes to the `fdemon doctor` exit code.
///
/// - Android components gate only when the Android SDK is present (`android_gates`).
/// - `WebBrowser` is **never** gating: a browser is optional, mirroring the wizard's
///   non-blocking `Missing → Partial` treatment of the Web leaf. It is printed for
///   information but does not fail the exit code.
/// - All other (core) components always gate.
fn component_gates(kind: &ComponentKind, android_gates: bool) -> bool {
    if is_android_component(kind) {
        android_gates
    } else if matches!(kind, ComponentKind::WebBrowser) {
        false
    } else {
        true
    }
}
```

Replace the inline `let gates = …` in `run_doctor` with `let gates = component_gates(&c.kind, android_gates);`.
The `println!` of every component row (`:108`) is unchanged — WebBrowser still appears in the output.

#### 2. Update the module doc

`src/doctor.rs:9–19` ("Exit-code gating rules") documents core (always gate) and Android (gate only when
SDK present). Add a short paragraph: **`WebBrowser` is non-gating** — a missing browser is optional and
surfaces in the listing but never fails the exit code, consistent with the wizard treating a missing web
browser as a non-blocking `Partial`.

### Acceptance Criteria

1. `component_gates(&ComponentKind::WebBrowser, _)` returns `false` for both `android_gates` values.
2. `component_gates` returns `android_gates` for the 5 Android kinds and `true` for core kinds
   (`FlutterSdk`, `Git`, `Jdk`, `Prerequisites`).
3. A report with `WebBrowser == Missing` and all other components `Ok` does **not** set `all_ok = false`
   (verified via `component_gates` + `is_failing`, the same helper-level pattern the existing tests use —
   `run_doctor` itself does real I/O and is not directly unit-tested).
4. WebBrowser still appears in the printed component listing (gating ≠ hiding).
5. Module doc documents the WebBrowser non-gating rule.
6. `cargo test -p flutter-demon --lib doctor` (or the binary's doctor tests) green; `cargo fmt --all` +
   `cargo clippy -- -D warnings` clean.

### Testing

```bash
cargo build
cargo test --lib doctor
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
```

New tests to add (mirror the existing helper-test style in `doctor.rs`'s `mod tests`):
- `component_gates_web_browser_never_gates` — WebBrowser → `false` for `android_gates ∈ {true,false}`.
- `component_gates_core_always_gates` — FlutterSdk/Git/Jdk/Prerequisites → `true`.
- `component_gates_android_follows_android_gates` — Android kinds → mirror `android_gates`.
- `web_browser_missing_does_not_fail_exit` — build a report (`make_report`) with all-Ok core + Android-absent
  + `WebBrowser` Missing; assert that folding `component_gates` + `is_failing` over the components keeps
  `all_ok == true`.
- Extend `is_android_component_classifies_correctly` with `assert!(!is_android_component(&ComponentKind::WebBrowser))`.

### Notes

- **Do not** alter the daemon's `check_web` (it must keep emitting raw `Missing` — non-blocking is a consumer
  policy, applied here for the doctor consumer, mirroring the wizard's leaf-local cap).
- Keep `is_failing` and `is_android_component` unchanged — only add `component_gates` and route the loop
  through it.
- The module-doc edit lives in `src/doctor.rs` (source `//!` doc, implementor-editable). The separate
  `docs/ARCHITECTURE.md` `fdemon doctor` entry is Task 06 (`doc_maintainer`).
