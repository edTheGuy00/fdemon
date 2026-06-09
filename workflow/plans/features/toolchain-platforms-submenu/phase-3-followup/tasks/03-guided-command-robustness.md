## Task: Guided-command robustness — best-effort framing + winget gating + dedup

**Objective**: Reduce guided-command "drift" risk: the per-OS web-browser guided commands hardcode package
manager invocations that can be wrong (Ubuntu `apt install chromium-browser` is a snap transitional;
Debian uses `chromium`; `yay -S google-chrome` assumes an AUR helper; `winget install Google.Chrome`
assumes winget is present). Lead with the cross-distro-robust `CHROME_EXECUTABLE` + download fallback,
gate the Windows winget command on `report.winget_available`, and extract the duplicated note suffix.

**Depends on**: Phase 3 (merged). Review findings C (MEDIUM) + K (LOW).

**Agent:** implementor

**Estimated Time**: 1.5–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/install_wizard/state.rs` — `web_browser_guided_commands` + tests.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/types.rs` — `HostPlatform`, `LinuxPackageManager`, `ToolchainReport` (note `winget_available: bool`).

### Details

> Line numbers are a current snapshot and will drift — locate by symbol.

`web_browser_guided_commands(report, web_status)` lives at `state.rs:570`. It already returns empty unless
`web_status == Partial` (correct, from the Phase-3 integration fix). This task improves the *content* of the
commands it emits.

#### K. Extract duplicated literals into `const`s

Every Linux arm repeats the suffix
`"…\nTo use a non-default browser: export CHROME_EXECUTABLE=\"/path/to/browser\""` and the Chrome download
URL `https://www.google.com/chrome/`. Hoist to module-level consts, e.g.:

```rust
const CHROME_DOWNLOAD_URL: &str = "https://www.google.com/chrome/";
const CHROME_EXECUTABLE_NOTE: &str =
    "To use a non-default browser: export CHROME_EXECUTABLE=\"/path/to/browser\"";
```

Use them across the Linux/macOS/Windows arms so the five-way note duplication collapses to one source.

#### C. Best-effort framing + cross-distro robustness

- **Lead with the robust path.** For every platform where the browser is `Partial`, ensure the guided list
  includes the `export CHROME_EXECUTABLE="…"` option (Linux/macOS) or `set CHROME_EXECUTABLE=…` (Windows)
  and the Chrome download URL as a reliable fallback that does not depend on a package manager. The
  package-manager command remains a convenience hint, not the only option.
- **Linux Debian/apt nuance.** The `LinuxPackageManager::Apt` arm should not present
  `apt install chromium-browser` as authoritative (snap transitional on Ubuntu; `chromium` on Debian).
  Keep a package hint but make the note acknowledge distro variance and point to `CHROME_DOWNLOAD_URL` +
  `CHROME_EXECUTABLE_NOTE`. (Do not attempt per-distro detection — that is out of scope; framing only.)
- **Windows winget gating.** Gate the `winget install Google.Chrome` command on `report.winget_available`
  (already computed in the report — reading it in `build_steps`/this helper is pure, no I/O). When winget is
  unavailable, emit the `CHROME_DOWNLOAD_URL` + `set CHROME_EXECUTABLE` path instead. `web_browser_guided_commands`
  may need the `report.winget_available` field (it already takes `&ToolchainReport`, so no signature change).

Keep the function pure and the Ok/Pending → empty behaviour unchanged (only `Partial` emits commands).

### Acceptance Criteria

1. The `CHROME_EXECUTABLE` note suffix and Chrome download URL exist as single `const`s (no 5× literal
   duplication).
2. Every `Partial` platform path offers a package-manager-independent fallback (`CHROME_EXECUTABLE` +
   download URL), not only a `sudo`/`winget` command.
3. The Windows arm emits `winget install Google.Chrome` only when `report.winget_available == true`; when
   false it falls back to the download URL + `set CHROME_EXECUTABLE`.
4. Ok / Pending web status still yields an empty command list (unchanged from the Phase-3 fix).
5. `cargo test -p fdemon-app --lib install_wizard::state` green; `cargo fmt --all` +
   `cargo clippy -p fdemon-app --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-app --lib install_wizard::state
cargo test -p fdemon-app --lib
cargo fmt --all && cargo clippy -p fdemon-app --all-targets -- -D warnings
```

New/updated tests:
- `web_guided_windows_uses_winget_when_available` — `winget_available = true` → command list contains the
  winget command.
- `web_guided_windows_falls_back_when_no_winget` — `winget_available = false` → no winget command; download
  URL + `set CHROME_EXECUTABLE` present.
- `web_guided_partial_always_offers_chrome_executable_fallback` — each platform's `Partial` output includes
  the `CHROME_EXECUTABLE` option.
- Keep/extend the existing `test_web_no_guided_command_when_browser_ok` and the Partial-has-commands test.

### Notes

- **Do not** touch `handler/install_wizard/actions.rs` (that file is unchanged in this followup) or
  `step_detail.rs`. Confine writes to `install_wizard/state.rs` so this parallelizes with Task 04.
- `report.winget_available` is data already on the report (pre-computed in `run_preflight`); reading it keeps
  `build_steps`/the helper pure-on-report — no settings param, no I/O.
- Per-distro detection (Debian `chromium` vs Ubuntu snap) is explicitly out of scope — this is framing +
  fallback robustness, tracked from review finding C as a known limitation otherwise.
