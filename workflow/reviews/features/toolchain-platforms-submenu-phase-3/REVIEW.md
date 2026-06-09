# Code Review: Phase 3 — Web leaf + `web_browser_executable`

**Feature:** toolchain-platforms-submenu Phase 3 (live Web platform leaf + browser detection)
**Branch:** `feat/toolchain-platforms-submenu`
**Diff base:** `git diff 078c2f3..HEAD -- . ':!workflow/plans/'` (14 files, ~1,114 insertions)
**Review date:** 2026-06-09
**Reviewers:** architecture_enforcer · code_quality_inspector · logic_reasoning_checker · risks_tradeoffs_analyzer · security_reviewer

## Verdict: ⚠️ NEEDS WORK

One blocking functional issue: the `fdemon doctor` CLI exit code does **not** honour the Phase-3
"Web never blocks" contract (flagged independently by `code_quality_inspector` and
`risks_tradeoffs_analyzer`, and confirmed during consolidation). Everything else is non-blocking polish.
Architecture, logic, and security all passed cleanly.

| Agent | Verdict |
|-------|---------|
| architecture_enforcer | ✅ PASS (1 suggestion) |
| code_quality_inspector | ⚠️ NEEDS WORK (1 major, several minor) |
| logic_reasoning_checker | ✅ PASS |
| risks_tradeoffs_analyzer | ⚠️ CONCERNS (1 high, several medium) |
| security_reviewer | ✅ PASS (2 medium, 2 low — no injection) |

---

## Strengths

- **TEA + layer boundaries are clean.** Web detection lives in `fdemon-daemon` (correct — no network/FS
  I/O in `app`); the configured override is plumbed via the established "pre-compute-in-preflight"
  pattern (`RunToolchainPreflight` → `run_preflight` → embedded `WebBrowser` component), so `build_steps`
  stays pure-on-report with no signature churn. Both `ComponentKind` exhaustive matches updated.
- **The non-blocking contract holds *within the wizard*.** `Missing → Partial` is capped locally at the
  Web leaf (`rollup_status` unchanged, so Android keeps true `Missing`); the Platforms parent rolls up to
  at most `Partial`; `flutter_now_live` / `close_wizard_and_dispatch_discovery` read only `FlutterSdk` and
  are provably unaffected.
- **The prior 02+03 integration defect is correctly closed.** Guided commands now emit **only** for
  `Partial` (the `empty → Pending` guard runs *before* the cap; `web_status != Partial → return empty`).
  Verified by full logic trace.
- **No security exposure.** The `probe_version` subprocess uses `Command::new(path).arg("--version")` —
  shell-free by construction, hardcoded arg, `is_file()`-gated. Guided commands are static per-OS string
  literals (copy-paste only, never executed). The config value is only ever a probe target.

---

## Findings (consolidated & deduplicated)

### 🔴 MAJOR — must fix before merge

#### A. `fdemon doctor` exits 1 on a browser-less host — contradicts the non-blocking contract
- **Source:** code_quality_inspector (MAJOR) + risks_tradeoffs_analyzer (HIGH) — *confirmed during consolidation*
- **File:** `src/doctor.rs:93–103`
- **Problem:** The `Missing → Partial` cap exists only in `build_steps` (app layer). `run_doctor` reads
  the **raw** preflight report, and its gating treats every non-Android component as a hard gate
  (`gates = true`). `WebBrowser` is not in `is_android_component`, so a raw `Missing` browser sets
  `all_ok = false` and the command exits `1`. CI containers / headless Linux servers without Chrome —
  exactly the case Phase 3 set out not to block — now fail `fdemon doctor` even when Flutter + Android are
  fully healthy. The plan's "never blocks" verification only checked `flutter_now_live` /
  `close_wizard_and_dispatch_discovery`, missing this path.
- **Fix:** Exempt `ComponentKind::WebBrowser` from exit-code gating (mirror the `android_gates` pattern —
  still print it for information). Add a regression test: a report with a `Missing` WebBrowser and
  otherwise-Ok components must exit `0`. Update the `run_doctor` module doc to note Web is non-gating.

### 🟡 MEDIUM — should fix / track

#### B. Platform-specific detection arms (macOS/Windows) are untested on the Linux CI
- **Source:** risks_tradeoffs_analyzer + code_quality_inspector
- **File:** `crates/fdemon-daemon/src/toolchain/checks/web.rs:136–183`
- **Problem:** `find_browser_macos` / `find_browser_windows` only execute on their host OS; Linux CI never
  runs them. A typo in a macOS bundle path or a Windows env-var name would pass CI and only fail in the
  field (silent false-negative "browser installed but reported Missing"). Graceful degradation prevents a
  crash, not the wrong result.
- **Fix:** Extract the candidate-path lists into `const`s and unit-test them with a fixed
  `HostPlatform::MacOs`/`Windows` (+ a tempdir-injected path where `is_file` is involved), so the per-OS
  logic is exercised cross-host.

#### C. Guided-command distro/tool drift
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/install_wizard/state.rs:570–662`
- **Problem:** `apt install chromium-browser` is a snap-transitional on modern Ubuntu and the wrong package
  on Debian (`chromium`); `yay -S google-chrome` assumes an AUR helper; `winget install Google.Chrome`
  assumes winget is present. These are static strings that can't track distro reality.
- **Fix (track, not blocking):** Frame package-manager commands as best-effort hints and lead with the
  cross-distro-robust `export CHROME_EXECUTABLE` + download-URL fallback. Optionally gate the Windows
  winget command on the already-available `report.winget_available` (data is in the report — no purity
  violation).

#### D. `web_browser_executable` Rust doc comment is inaccurate ("Sets `CHROME_EXECUTABLE`")
- **Source:** security_reviewer (MEDIUM)
- **File:** `crates/fdemon-app/src/config/types.rs` (the `web_browser_executable` field doc comment)
- **Problem:** The field is a wizard-probe override; it does **not** call `set_var` and does **not**
  propagate to Flutter's own `flutter run -d chrome` process. The `docs/CONFIGURATION.md` / `ARCHITECTURE.md`
  prose was already corrected (`468b3e1`) to "takes precedence over…", but the **source doc comment** was
  missed. A user could set this and wrongly expect `flutter run -d chrome` to honour it.
- **Fix:** Align the source doc comment with the corrected `.md` wording (probe-only; does not set the env
  var for Flutter's processes).

### 🟢 LOW — optional polish

| # | Finding | Source | File |
|---|---------|--------|------|
| E | `probe_version` takes `&PathBuf`; idiom is `&Path` | architecture_enforcer, code_quality | `checks/web.rs:190` |
| F | Tautological test assertion (`… || !detail.is_empty()` always true) | code_quality | `checks/web.rs:262–264` |
| G | Non-`#[serial]` tests also read global `CHROME_EXECUTABLE` → potential CI race | code_quality | `checks/web.rs` (`test_check_web_respects_browser_override`, `…_nonexistent_override_falls_through`) |
| H | `probe_version` 10 s timeout over-generous for `--version` (use 3–5 s) | risks, security | `checks/web.rs` / `checks/mod.rs:65` |
| I | Fixed count+index assertion is a Phase-4 tripwire | risks | `toolchain/mod.rs:260–276` — prefer presence assertions or add a `// Phase 4: host-variable` forward-pointer |
| J | `web_browser_executable` is an unvalidated free-form string (safe today: `is_file` + no shell) | security | `config/types.rs` — optional length/null-byte cap at parse time |
| K | DRY: the `CHROME_EXECUTABLE` note suffix is duplicated 5× | code_quality | `state.rs:570–662` — extract a `const` |
| L | `linux_package_manager.unwrap_or(Unknown)` masks an invariant (unreachable in practice) | code_quality | `state.rs:583–585` |
| M | `step_caption(PlatformWeb)` returns `Some` unconditionally (cosmetic; caption shows even when browser is Ok) | logic | `step_detail.rs:98` — optional symmetry with the JDK caption |

### ⚪ Dismissed (false positive)

- **`step_detail.rs:2116` "malformed comment"** (code_quality, NITPICK) — **incorrect**. The line is a
  well-formed `//` comment; `cargo clippy -p fdemon-tui --all-targets -- -D warnings` is clean. No action.

---

## Documentation Freshness

✅ `docs/ARCHITECTURE.md` and `docs/CONFIGURATION.md` were updated in this diff (and the factual errors
from the first doc pass were already corrected in `468b3e1`). The **one** remaining doc gap is the
inaccurate **source** doc comment in `config/types.rs` (Finding D) — the prose docs were fixed but the
Rust doc comment was not.

---

## Recommendation

The implementation is high quality and architecturally sound — but **Finding A is a genuine functional
regression** against the phase's own headline contract and must be fixed before merge (it's a small,
well-understood change: exempt `WebBrowser` from `doctor.rs` gating + one regression test). Findings B and
D are worth folding into the same fix pass; C and the LOW items can be a tracked follow-up.

See `ACTION_ITEMS.md` for the actionable checklist.
