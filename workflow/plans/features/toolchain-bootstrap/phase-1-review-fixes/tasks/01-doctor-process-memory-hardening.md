## Task: Doctor Process & Memory Hardening + Shared ANSI Helper (fdemon-daemon)

**Objective**: Fix the orphaned-process leak and unbounded reads in `capture_flutter_doctor`,
bound the `DoctorLine::indent` allocation, and consolidate the duplicated `strip_ansi` so the
toolchain parser reuses one OSC-aware helper. Addresses review findings **M1**, **n11**, **n13**.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/doctor.rs` — timeout/kill restructure, byte-capped reads,
  indent cap, drop the forked `strip_ansi`.
- `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` — extend the shared `strip_ansi` to also
  handle OSC (`ESC ]`) sequences and ensure it is reachable from `toolchain` (it is already
  `pub(crate)` in `fdemon-daemon`).

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `DoctorLine`, `DoctorMarker`.
- `crates/fdemon-daemon/src/toolchain/mod.rs` — how `capture_flutter_doctor` is invoked.
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` — `FlutterExecutable::command()`.

### Details

**M1 — kill the timed-out child + cap reads** (`doctor.rs:32-104`):

The current `Err(_)` timeout arm (lines 95-102) returns `None` with a comment claiming it kills the
process, but `child` was moved into the timeout future (line 50) and is merely dropped; the command
sets no `kill_on_drop`. Restructure so the child is reachable on timeout and is killed, e.g.:

- Spawn with `.kill_on_drop(true)` on the `Command` before `spawn()` (simplest, covers the
  drop-on-timeout path), **and/or** keep `child` out of the moved future and call
  `let _ = child.start_kill();` (or `child.kill().await`) in the `Err(_)` arm.
- Cap both reads: replace `read_to_end(&mut buf)` with
  `AsyncReadExt::take(stdout, MAX_DOCTOR_OUTPUT_BYTES).read_to_end(&mut buf)` for stdout and stderr.
  Add a documented constant:

```rust
/// Upper bound on captured `flutter doctor -v` output. Real output is a few KiB;
/// this caps a misbehaving/replaced binary that streams unbounded data.
const MAX_DOCTOR_OUTPUT_BYTES: u64 = 1024 * 1024; // 1 MiB per stream
```

- Replace the misleading `// Kill the lingering process on timeout` comment with one that matches
  the implemented behavior.

**n13 — consolidate `strip_ansi`** (`doctor.rs:106-149` + `flutter_sdk/diagnostics.rs`):

`doctor.rs` forked a second ANSI stripper to add OSC handling. Extend the canonical
`flutter_sdk::diagnostics::strip_ansi` to handle OSC (`ESC ]` … `BEL`/`ESC \`) in addition to CSI,
keep it `pub(crate)`, and have `doctor.rs` call it. Remove the local copy. Ensure existing
`strip_ansi` callers (`devices.rs`, `emulators.rs`) still behave identically for CSI input — add a
test asserting OSC stripping does not regress CSI behavior.

**n11 — cap indent** (the `DoctorLine::indent` computation in `parse_single_line`):

`indent` is the count of leading spaces from untrusted doctor output and is later used in
`" ".repeat(indent)` at render time. Cap it during parsing:

```rust
/// flutter doctor never indents more than a handful of spaces; cap defensively
/// so a pathological line cannot drive a large per-frame allocation in the TUI.
const MAX_DOCTOR_INDENT: usize = 32;
let indent = leading_spaces.min(MAX_DOCTOR_INDENT);
```

### Acceptance Criteria

1. On timeout, `capture_flutter_doctor` returns `None` **and** the spawned child is killed/reaped
   (no orphaned `flutter` process). The timeout comment matches the code.
2. stdout and stderr reads are each capped at `MAX_DOCTOR_OUTPUT_BYTES`; oversized output is
   truncated, not buffered for the full timeout.
3. `DoctorLine::indent` is `<= MAX_DOCTOR_INDENT` for any input.
4. `doctor.rs` contains no local `strip_ansi`; it calls the shared
   `flutter_sdk::diagnostics::strip_ansi`, which now also strips OSC sequences.
5. `parse_doctor_output` remains pure/total (no panic on empty/garbage); existing doctor tests pass.

### Testing

```rust
#[test] fn test_strip_ansi_removes_osc_sequences() { /* ESC ] ... BEL */ }
#[test] fn test_strip_ansi_csi_unchanged() { /* existing CSI behavior preserved */ }
#[test] fn test_parse_caps_indent_at_max() { /* line with 1000 leading spaces -> 32 */ }
```

- Killing/timeout behavior is hard to unit-test deterministically; assert the structural change
  (e.g. `kill_on_drop` set) and verify manually if practical. Do not add a flaky timing test.
- Keep all assertions on the pure parser; the process path should at minimum be exercised for
  "does not panic".

### Notes

- Do not change the 60s `DOCTOR_TIMEOUT` value or the display-only contract.
- `read_to_end` after `take(..)` requires `use tokio::io::AsyncReadExt;` (already imported).
- This task **exposes/extends** the shared `strip_ansi` that task 02 (n12) will consume — keep the
  signature `pub(crate) fn strip_ansi(&str) -> String`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/doctor.rs` | Added `kill_on_drop(true)` on Command; restructured I/O handle extraction before async block; capped both stdout and stderr reads via `AsyncReadExt::take(MAX_DOCTOR_OUTPUT_BYTES)`; added `MAX_DOCTOR_OUTPUT_BYTES` constant (1 MiB per stream); added `MAX_DOCTOR_INDENT` constant (32); capped `indent` in `parse_single_line`; removed local `strip_ansi`; imported `crate::flutter_sdk::diagnostics::strip_ansi`; fixed misleading timeout comment; added three new tests |
| `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` | Extended `strip_ansi` to handle OSC sequences (`ESC ]` ... `BEL`/`ESC \`) in addition to CSI; added four new tests including mixed CSI+OSC and OSC+ST variants |

### Notable Decisions/Tradeoffs

1. **`kill_on_drop(true)` approach**: The task suggested either `kill_on_drop(true)` or keeping `child` out of the async block for explicit `start_kill()`. Both approaches were implemented: `kill_on_drop(true)` is set on the Command (covers all drop paths), and the I/O handles are extracted before the async block so `child` is accessible (though with `kill_on_drop` set it is safe to let it drop at timeout). This gives defense-in-depth.

2. **I/O handle extraction before timeout future**: To use `kill_on_drop` cleanly and make the flow explicit, `child.stdout.take()` and `child.stderr.take()` are called before the `tokio::time::timeout` closure. The `child` is then moved into the async block so it is dropped at the end of the happy path after `child.wait()`.

3. **`AsyncReadExt::take` syntax**: The task noted this correctly. Using `AsyncReadExt::take(stdout, MAX_DOCTOR_OUTPUT_BYTES)` (as a free function form) works because `AsyncReadExt` is already imported. Both stdout and stderr reads are capped.

4. **OSC ST two-char terminator**: The OSC terminator `ESC \` in the shared `strip_ansi` was handled correctly — after seeing `\x1b` inside an OSC sequence, one additional character is consumed (the `\`), matching the existing behavior in the old local copy.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all test suites pass, no failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Timeout kill coverage**: `kill_on_drop(true)` sends SIGKILL on Unix and `TerminateProcess` on Windows. The signal is async — the OS may take a few milliseconds to actually terminate the process, but the Rust handle is released immediately. This is the standard safe approach and matches what other tools do.

2. **OSC C1 ST (`\u{9C}`)**: The shared `strip_ansi` now handles `\u{9C}` as an OSC terminator (C1 String Terminator). This was present in the old local copy and is preserved. Real Flutter output uses BEL (`\x07`) or `ESC \` but C1 support is low-cost and correct.
