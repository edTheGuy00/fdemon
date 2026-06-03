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
