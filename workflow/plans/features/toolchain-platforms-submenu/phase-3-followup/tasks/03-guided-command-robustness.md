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

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/install_wizard/state.rs` | Added `CHROME_DOWNLOAD_URL` + `CHROME_EXECUTABLE_NOTE` consts; refactored `web_browser_guided_commands` to emit the download-URL fallback on every platform arm, gate Windows winget on `report.winget_available`, use the consts everywhere; updated doc comment; added 4 new tests |

### Notable Decisions/Tradeoffs

1. **Linux arms now emit 2 commands (PM hint + download URL)** instead of 1: the PM-specific `sudo install` command is retained as a convenience (first command), and the cross-distro `CHROME_DOWNLOAD_URL` fallback is always appended as the second. This matches the criterion of "not only a PM command".
2. **Apt note updated for distro variance**: The Apt arm's note now acknowledges that `chromium-browser` is the Ubuntu name and `chromium` is the Debian name, explicitly pointing to `CHROME_DOWNLOAD_URL` as the safe fallback — without attempting per-distro detection (out of scope).
3. **Windows winget gating**: When `report.winget_available == false`, only the download-URL command is emitted. When `true`, the winget command is prepended as a convenience, and the download URL follows. This satisfies criterion 2 (the failed case from the prior attempt).
4. **macOS note extended**: The macOS note now incorporates `CHROME_EXECUTABLE_NOTE` via `format!`, eliminating one more inline duplicate.
5. **`map_or(false, …)` replaced with `.is_some_and(…)`** in the new tests after clippy flagged them.

### Testing Performed

- `cargo test -p fdemon-app --lib install_wizard::state` — Passed (144 tests)
- `cargo test -p fdemon-app --lib` — Passed (3002 tests)
- `cargo fmt --all` — Clean
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Clean

### Risks/Limitations

1. **Linux PM-specific command label changed**: Now says "Install a browser (package manager)" instead of "Install a browser". This is visible in the TUI. The TUI rendering does not assert exact label text, so no TUI tests break — but users will see the updated label.
2. **macOS now emits two `CHROME_EXECUTABLE` hints in the note**: The note includes the app-specific path (`/Applications/Google Chrome.app/...`) and the generic `CHROME_EXECUTABLE_NOTE` line. Minor redundancy, but explicitly comprehensive.
3. **Per-distro detection** (Debian `chromium` vs Ubuntu snap transitional for Apt) remains out of scope and is noted in the Apt arm's note text.
