## Task: Daemon web.rs hardening — testable detection + probe polish

**Objective**: Harden `checks/web.rs`: make the macOS/Windows detection arms testable cross-host (they
are currently compiled-but-never-executed on the Linux CI), give the browser `--version` probe a dedicated
short timeout, fix the `&PathBuf` parameter, remove a tautological test assertion, and serialize all tests
that read the global `CHROME_EXECUTABLE` env var.

**Depends on**: Phase 3 (merged). Review findings B, E, F, G, H.

**Agent:** implementor

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/checks/web.rs` — detection refactor + consts + probe timeout + tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/checks/mod.rs` — `PROBE_TIMEOUT`, `JDK_PROBE_TIMEOUT` (timeout precedent), `strip_and_truncate`.

### Details

> Line numbers are a current snapshot and will drift — locate by symbol/test-name.

#### B. Make macOS/Windows detection testable cross-host

Today `find_browser_macos` (`:136`) and `find_browser_windows` (`:152`) inline their candidate paths and
probe the real filesystem/env, so the Linux CI compiles but never executes them — a typo in a bundle path
or a wrong env-var name would ship silently.

- **macOS** (`find_browser_macos`): hoist the candidate list to a module-level (or `pub(super)`) const,
  e.g. `const MACOS_BROWSER_CANDIDATES: &[&str] = &[...]`. Add a unit test asserting the const contains the
  expected canonical bundle paths (catches typos cross-host). Keep `find_browser_macos` iterating the const
  + `is_file()` (the `is_file` probe stays host-dependent, but the path strings are now verified).
- **Windows** (`find_browser_windows`): extract a **pure** helper that takes the env values as parameters
  so it is testable on any host, e.g.
  `fn windows_chrome_candidates(program_files: Option<&str>, local_app_data: Option<&str>) -> Vec<PathBuf>`
  building the `Google\Chrome\Application\chrome.exe` paths. `find_browser_windows` calls it with
  `std::env::var(...).ok().as_deref()` then `is_file()`-filters, plus the existing `which::which("msedge")`
  fallback. Unit-test `windows_chrome_candidates` with injected values (assert the joined paths) and an
  injected tempfile path that exists (assert it is returned by a small `first_existing` helper if you
  factor one out). Keep the `msedge` fallback behaviour unchanged.

The Linux arm already uses a `CANDIDATES` const (`:117`) — leave it, optionally add a const-content test for
parity.

#### H. Dedicated browser version-probe timeout

`probe_version` (`:190`) currently uses the shared `PROBE_TIMEOUT` (10s, `checks/mod.rs:67`). A browser
`--version` should be near-instant; 10s is over-generous if a wrapper script hangs. Follow the existing
`JDK_PROBE_TIMEOUT` precedent (a local `const` in the probe's module): add
`const BROWSER_VERSION_TIMEOUT: Duration = Duration::from_secs(5);` local to `web.rs` and use it in
`probe_version`'s `tokio::time::timeout`. (Keep it `web.rs`-local so this task does not touch `checks/mod.rs`.)

#### E. `&PathBuf` → `&Path`

`probe_version(browser_path: &PathBuf)` (`:190`) → `browser_path: &Path` (idiomatic; `Command::new` accepts
`AsRef<OsStr>`; all call sites Deref-coerce). Add `use std::path::Path;` if needed.

#### F. Remove tautological test assertion

`test_check_web_respects_browser_override` (`:244`) asserts
`result.detail.contains(path) || !result.detail.is_empty()` — the second disjunct is always true (the
fallback guarantees non-empty detail), so the path check never bites. Replace with a meaningful assertion:
the status is `Ok` and `detail` is non-empty (and, if the test binary's `--version` behaviour permits a
deterministic check, that `detail` references the override path). Do not weaken the `Ok`-on-override
invariant.

#### G. Serialize env-reading tests

`check_web` reads `CHROME_EXECUTABLE` (`:65`). `test_check_web_respects_chrome_executable_env` (`:287`) is
already `#[serial_test::serial]`, but `test_check_web_respects_browser_override` (`:244`) and
`test_check_web_nonexistent_override_falls_through` (`:271`) also call `check_web` (and thus read the env
var) without `#[serial]`, so they can race the env-mutating test on a parallel runner. Add
`#[serial_test::serial]` to both.

### Acceptance Criteria

1. macOS candidate paths and Windows path-construction are exercised by host-agnostic unit tests (a typo in
   a path/env-var name would now fail CI on Linux).
2. `probe_version` takes `&Path` and uses `BROWSER_VERSION_TIMEOUT` (≤5s), not `PROBE_TIMEOUT`.
3. The override test no longer contains a tautological assertion.
4. All `check_web`-calling tests are `#[serial]` (no `CHROME_EXECUTABLE` race).
5. `check_web` behaviour is unchanged on the live Linux host (override → env → PATH probe → Missing/Unknown).
6. `cargo test -p fdemon-daemon --lib toolchain::checks::web` green; `cargo fmt --all` +
   `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-daemon --lib web
cargo test -p fdemon-daemon --lib
cargo fmt --all && cargo clippy -p fdemon-daemon --all-targets -- -D warnings
```

New/updated tests:
- `macos_browser_candidates_are_canonical_bundle_paths` (const content).
- `windows_chrome_candidates_builds_program_files_and_localappdata_paths` (injected env values).
- `windows_chrome_candidates_finds_injected_tempfile` (tempdir + injected value).
- Updated `test_check_web_respects_browser_override` (non-tautological).
- `#[serial]` added to the two override tests.

### Notes

- This task is confined to `checks/web.rs` so it parallelizes with Task 05 (which edits
  `toolchain/mod.rs`) in the same daemon crate.
- Keep the env-var save/restore guard pattern for any test that mutates `CHROME_EXECUTABLE`.
- Do not change the public `check_web` signature or its Ok/Missing/Unknown semantics — this is a
  testability + polish pass, not a behaviour change.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a7b8e7ede0027a4e3

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/toolchain/checks/web.rs` | All 5 changes: MACOS_BROWSER_CANDIDATES const + windows_chrome_candidates helper + BROWSER_VERSION_TIMEOUT + &Path fix + serial attrs + tautological assertion fix + 5 new tests |

### Notable Decisions/Tradeoffs

1. **`pub(super)` visibility for new const and helper**: `MACOS_BROWSER_CANDIDATES` and `windows_chrome_candidates` are `pub(super)` so the test module inside `web.rs` can reach them directly. This keeps them private to `checks/` while enabling the cross-host unit tests.

2. **BROWSER_VERSION_TIMEOUT kept web.rs-local**: Follows the same pattern as `JDK_PROBE_TIMEOUT` in `mod.rs` — module-specific constants stay in their own module. No changes to `checks/mod.rs`.

3. **Tautological assertion replaced**: The `result.detail.contains(path) || !result.detail.is_empty()` assertion was replaced with simply `!result.detail.is_empty()`, which is the invariant that actually matters: the fallback always guarantees a non-empty detail when the override path exists.

4. **`&PathBuf` → `&Path`**: `probe_version` now takes `&Path`. All three call sites pass `&path` / `&env_path` which Deref-coerce correctly; no call sites required changes.

5. **Pre-existing test failure**: `toolchain::tests::test_run_preflight_nonexistent_sdk_path_does_not_panic` was already failing before this task (confirmed by git stash test). This is the subject of Task 05 (count-assertion-forward-compat).

### Testing Performed

- `cargo test -p fdemon-daemon --lib web` — 11 passed (including 5 new tests)
- `cargo fmt --all` — clean
- `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` — clean

### Risks/Limitations

1. **Windows path construction on Linux**: The `windows_chrome_candidates` tests use forward-slash-compatible path joining (`PathBuf::join`). On Linux, `PathBuf::from("C:\\Program Files").join("Google")` joins with `/` not `\`, so the path comparison tests verify the join semantics portably. This is intentional — we are testing path-string typos, not Windows path separators.
