# Action Items: Phase 3 — Web leaf + `web_browser_executable`

**Review Date:** 2026-06-09
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 1

## Critical / Major Issues (Must Fix)

### 1. `fdemon doctor` exits 1 on a browser-less host (breaks the non-blocking Web contract)
- **Source:** code_quality_inspector (MAJOR), risks_tradeoffs_analyzer (HIGH) — confirmed during consolidation
- **File:** `src/doctor.rs:93–103`
- **Problem:** The `Missing → Partial` non-blocking cap lives only in `build_steps` (app layer).
  `run_doctor` reads the raw preflight report and gates the exit code with `gates = true` for every
  non-Android component. `WebBrowser` is not in `is_android_component`, so a raw `Missing` browser sets
  `all_ok = false` and the CLI exits `1` — on exactly the headless/CI hosts Phase 3 promised not to block.
- **Required Action:** Exempt `ComponentKind::WebBrowser` from exit-code gating in `run_doctor` (mirror the
  `android_gates` exemption; keep printing the row for information). Update the `run_doctor` module doc to
  state Web is non-gating.
- **Acceptance:**
  - New test: a report with `WebBrowser` = `Missing` and all other components `Ok` → `run_doctor` exit `0`.
  - `is_android_component_classifies_correctly` (or a sibling) asserts `WebBrowser` is non-gating.
  - `cargo test --workspace --lib` green.

## Should-Fix (fold into the same pass)

### 2. macOS/Windows detection arms are untested on Linux CI
- **Source:** risks_tradeoffs_analyzer, code_quality_inspector
- **File:** `crates/fdemon-daemon/src/toolchain/checks/web.rs:136–183`
- **Suggested Action:** Extract the macOS/Windows candidate-path lists into `const`s and add unit tests
  driven by a fixed `HostPlatform::MacOs`/`Windows` (+ tempdir-injected path for `is_file` paths), so the
  per-OS logic is exercised cross-host.

### 3. Inaccurate source doc comment on `web_browser_executable`
- **Source:** security_reviewer
- **File:** `crates/fdemon-app/src/config/types.rs` (field doc comment — says "Sets `CHROME_EXECUTABLE`")
- **Suggested Action:** Align with the corrected `.md` wording: probe-only override for the Install Wizard;
  does **not** set `CHROME_EXECUTABLE` for Flutter's own `flutter run -d chrome` process.

## Minor / Optional (track as follow-up)

| # | Item | File |
|---|------|------|
| 4 | Frame package-manager guided commands as best-effort; lead with `CHROME_EXECUTABLE` + download URL; optionally gate winget on `report.winget_available` | `install_wizard/state.rs:570–662` |
| 5 | `probe_version(&PathBuf)` → `&Path` | `checks/web.rs:190` |
| 6 | Remove tautological `|| !detail.is_empty()` from `test_check_web_respects_browser_override` | `checks/web.rs:262–264` |
| 7 | Add `#[serial_test::serial]` to the two non-serial tests that call `check_web` (read global `CHROME_EXECUTABLE`) | `checks/web.rs` |
| 8 | Dedicated 3–5 s timeout for the browser `--version` probe | `checks/web.rs` |
| 9 | Convert `toolchain/mod.rs` count+index assertions to presence-based, or add a `// Phase 4: host-variable` forward-pointer | `toolchain/mod.rs:260–276` |
| 10 | Optional: length/null-byte cap on `web_browser_executable` at config parse | `config/types.rs` |
| 11 | Extract the duplicated `CHROME_EXECUTABLE` note suffix into a `const` | `install_wizard/state.rs` |
| 12 | Optional: gate `step_caption(PlatformWeb)` on guided-command presence (symmetry with JDK caption) | `step_detail.rs:98` |

## Re-review Checklist

- [ ] Item 1 resolved — `fdemon doctor` exits 0 with only a missing browser (+ regression test)
- [ ] Items 2 & 3 resolved or explicitly deferred
- [ ] Minor items triaged into a follow-up task or closed
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all green

## Note

The 5-agent review did **not** dispatch a false-positive: `step_detail.rs:2116` was alleged to have a
malformed comment, but it is a valid `//` comment and the build is clean — no action.
