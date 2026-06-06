## Task: Make `fdemon doctor` a usable CI gate; reject ignored top-level flags; fix stderr dedup (F-PR53-11/14/15)

**Severity:** MEDIUM (correctness — CI gate) + two LOW

**Objective**: Fix the `fdemon doctor` CLI so non-Android Flutter projects can pass
in CI, so doctor-incompatible top-level run flags are not silently ignored, and so
`flutter doctor` stderr is not over-eagerly discarded.

**Depends on**: — (disjoint; safe to parallelize)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `src/doctor.rs`
- `crates/fdemon-daemon/src/toolchain/mod.rs`
- `src/main.rs`
- `crates/fdemon-daemon/src/toolchain/doctor.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/android.rs` (Android components return Missing/Unknown when no SDK)

### Details

**(a) Exit code is unconditionally strict (MEDIUM).**
`src/doctor.rs:39-43` sets `all_ok = false` for **any** component whose status
!= `Ok` (incl. Missing/Unknown), and 58-62 returns exit 1. `run_preflight`
(`toolchain/mod.rs:140-150`) always assembles 5 Android components regardless of
project type, and those return Unknown/Missing when no Android SDK is configured
(android.rs:132/234/239/276/322/368…). The module doc markets doctor as "intended
for CI pipelines / exit 0 when all OK". So a Flutter web/desktop/iOS-only project,
or any CI runner without an Android SDK, can **never** exit 0 — nullifying the
advertised CI use.

**(b) Ignored top-level run flags (LOW).**
`src/main.rs:24-54, 119-141`: `RunArgs` is `#[command(flatten)]`ed into `Cli`
without `global=true`. `fdemon --headless doctor`, `--dap-stdio doctor`,
`--log-dir X doctor` parse successfully but the doctor branch never reads
`cli.run`, so the flags are silent no-ops (verified empirically: exit 0, flags
ignored; `--log-dir` dir never created). `fdemon doctor --headless` correctly
errors (exit 2).

**(c) Over-eager stderr dedup (LOW).**
`toolchain/doctor.rs:104-111`: `if !combined.contains(stderr_str.trim_start())`
discards the entire stderr stream when its trimmed content is *any substring* of
stdout — can drop legitimate doctor diagnostics. Display-only impact.

### Proposed Fix

1. Treat Android components as **optional** for exit-code purposes: only fail the
   exit on non-Ok core components (FlutterSdk, Git, and JDK when relevant), and
   count Android failures only when an Android SDK root was actually resolved.
   Alternatively add a `--require-android` flag defaulting off. (Keep the printed
   report showing all components; only the exit-code aggregation changes.)
2. After detecting `Commands::Doctor`, error out (clap error / exit 2) if any
   `cli.run` flag was set; or scope those flags to the subcommand; or, at minimum,
   document that run flags are ignored with subcommands. Prefer erroring loudly.
3. Replace the substring `contains` dedup with exact equality
   (`combined.trim() != stderr_str.trim()`), or simply always append stderr.

### Acceptance Criteria

1. On a host/project with Flutter + Git OK but no Android SDK, `fdemon doctor`
   exits 0 (Android components shown but not gating); a genuinely broken core
   component still exits 1.
2. `fdemon --headless doctor` (and `--dap-stdio`, `--log-dir`) no longer silently
   succeed-and-ignore: either they error, or are accepted and honored — not silently
   dropped.
3. `flutter doctor` stderr is retained unless it is exactly duplicated by stdout.

### Testing

```rust
// src/doctor.rs / toolchain/mod.rs test module
// - exit-code: build a report with FlutterSdk/Git Ok and Android Unknown/Missing;
//     assert the aggregation yields success.
// - exit-code: a Missing FlutterSdk still yields failure.
// toolchain/doctor.rs test module
// - stderr dedup: stderr that is a substring (but not equal) of stdout is retained;
//     exactly-equal stderr is dropped.
// src/main.rs: a CLI-parse test (or documented integration check) asserting
//     `--headless doctor` is rejected / handled, not silently ignored.
```

### Notes

- File-disjoint from all other tasks (owns `src/doctor.rs`, `src/main.rs`,
  `toolchain/doctor.rs`, and the exit-aggregation use of `toolchain/mod.rs`) →
  Wave 1 parallel worktree candidate. Confirm `toolchain/mod.rs` is not written by
  another wave-peer (it is read by task 09 only; `run_preflight` shape is unchanged
  unless you choose to filter Android there — keep filtering in `src/doctor.rs` to
  avoid touching shared daemon code).
- The headline (a) is the medium item; (b) and (c) are low ergonomics/correctness nits bundled because they live in the same CLI surface.
