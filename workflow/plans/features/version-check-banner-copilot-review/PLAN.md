# Plan: Version-Check Banner — Copilot Review Follow-ups

**Status**: Draft
**Source**: 4 review comments from `copilot-pull-request-reviewer` on PR #49
(`feat(version-check): startup banner + hardening follow-ups`).
**Scope**: Surgical follow-ups only. No architectural change, no new feature.

---

## Goal

Close out the four Copilot review comments on PR #49 by fixing the two
real correctness gaps they surface (tag-string sanitization contract,
timeout=0-as-disable semantics) and aligning the doc comment that drifted
from the implementation.

---

## Per-comment verdict

### Comment 1 — security/contract: `check_for_newer_release` returns un-normalized tag string

**Verdict**: **VALID** — accept the fix.

**Evidence**:
- `parse_semver` (`crates/fdemon-app/src/version_check.rs:283`) splits on `['-', '+']` and parses only the prefix into `(u32, u32, u32)`:
  ```rust
  let core = s.split(['-', '+']).next()?;
  ```
  So `parse_semver("1.2.3-\x1b[31mEVIL")` returns `Some((1, 2, 3))`.
- `check_for_newer_release` returns the *original* `tag_str` (post-`v`-strip) on line 261:
  ```rust
  let result = if latest > current {
      Some(tag_str.clone())
  ```
  The `latest > current` check operates on the parsed triple, but the value handed back is the unfiltered `tag_str`.
- The doc on `check_for_newer_release` (lines 221-229) explicitly promises "digit-and-dot only" output and tells callers they can "skip escape-sequence sanitisation". That contract is **false** for any tag containing `-` or `+` followed by arbitrary bytes.
- The render site (`crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs:648-652`) trusts that contract — it interpolates `latest` directly into the banner string with no escaping:
  ```rust
  format!("\u{2B06} New version available: v{} (current v{}) — …", latest, env!("CARGO_PKG_VERSION"))
  ```
  ratatui's `Paragraph::new(text)` writes the text into terminal cells; embedded ESC bytes pass through to the terminal and are interpreted.
- Existing tests cover happy/`v`-prefix/malformed JSON/oversized/timeout but **do not** cover an ANSI/control-character suffix.

**Suggested fix**:
- Change `parse_semver` to **return both** the triple and the canonical `MAJOR.MINOR.PATCH` string (or wrap the existing helper), so callers can use the normalized form.
- Smallest concrete change: introduce a thin `parse_and_normalize_semver(s) -> Option<((u32,u32,u32), String)>` (or `parse_semver` returns a struct), and have `check_for_newer_release` return the **normalized** string built from the parsed components (`format!("{major}.{minor}.{patch}")`) — never the raw `tag_str`. The cache write should also use the normalized value.
- Update the doc comment on `check_for_newer_release` to keep the digit-and-dot promise.
- Add a regression test: feed a tag like `"1000.0.0-\x1b[31mEVIL\x1b[0m"` through `fetch_latest_tag` and verify `check_for_newer_release` either returns `None` or the normalized `"1000.0.0"` (whichever is the post-fix contract). The cache read path (which reuses `parse_semver` on cached strings) must also normalize before returning.

**Risk**: Low. Behavior change is narrow — only affects tags with `-`/`+` suffix, and the "newer-than" comparison already used the parsed triple. All existing tests stay green; one new test gets added.

---

### Comment 2 — doc accuracy: `fetch_latest_tag` doc claims "parseable" but doesn't validate

**Verdict**: **VALID** — accept the fix, fold into the same task as Comment 1.

**Evidence**:
- `fetch_latest_tag` doc (lines 146-148):
  > Returns the bare semver string (no `v` prefix) when the fetch succeeds and the remote tag is parseable; returns `None` on any error.
- Implementation (lines 199-207) only does `tag.strip_prefix('v')` — there is no semver validation.
- The "parseable" promise is therefore made by the doc but kept by the *caller* (`check_for_newer_release`), not by `fetch_latest_tag` itself.

**Suggested fix**:
- Once Comment 1's fix lands (normalization happens in `check_for_newer_release`), update `fetch_latest_tag`'s doc to accurately state: "Returns the remote `tag_name` with a single leading `v` stripped, or `None` on any I/O / HTTP / size error. Callers must validate the returned string before treating it as semver — this function does no semver parsing."
- No code change needed in `fetch_latest_tag` itself; the contract layering becomes: `fetch_latest_tag` = raw fetch, `check_for_newer_release` = validated/normalized public API.

**Risk**: None. Doc-only change.

---

### Comment 3 — semantics: `version_check_timeout_secs = 0` still spawns the check (run_with_project)

**Verdict**: **VALID** — accept the fix.

**Evidence**:
- `BehaviorSettings` doc comment (`crates/fdemon-app/src/config/types.rs:173`):
  > A value of 0 is equivalent to disabling the check.
- `docs/CONFIGURATION.md:264` (table) and `:320` (detail section) both repeat the claim:
  > A value of `0` disables the check (equivalent to setting `version_check = false`).
- `crates/fdemon-tui/src/runner.rs:78-85`:
  ```rust
  if engine.settings.behavior.version_check {
      spawn::spawn_version_check(
          engine.msg_sender(),
          std::time::Duration::from_secs(
              engine.settings.behavior.version_check_timeout_secs as u64,
          ),
      );
  }
  ```
  Only gates on the bool, not on the timeout. With `timeout_secs = 0`, `Duration::from_secs(0)` is passed to `reqwest::ClientBuilder::timeout`. reqwest still builds the client and starts the request — the request times out immediately, but DNS + TLS setup is initiated. Not truly "no outbound HTTP".

**Suggested fix**:
- Extend the gate to: `behavior.version_check && behavior.version_check_timeout_secs > 0`.
- The simplest, most readable form is an extra `&&` in the existing `if`. Adding a tiny helper `should_run_version_check(&BehaviorSettings) -> bool` co-located with `BehaviorSettings` is also acceptable and lets `run_with_project_and_dap` share the logic without duplication (resolves Comment 4 in the same change).

**Risk**: Low. Today `timeout_secs = 0` causes a quick failure that returns `None`; the user-visible outcome is identical (no banner). The fix just removes the brief network attempt. No tests should fail; one new unit test on the helper is appropriate.

---

### Comment 4 — duplicate of #3 at `run_with_project_and_dap`

**Verdict**: **VALID** — same root cause, same fix, **same task** as Comment 3.

**Evidence**:
- `crates/fdemon-tui/src/runner.rs:208-215` repeats the same `if engine.settings.behavior.version_check { spawn::spawn_version_check(...) }` block as the non-DAP runner.
- Both spawn paths are reachable; both need the timeout-gate change.

**Suggested fix**: Same as Comment 3 — best handled by introducing the shared helper so both call sites use it.

**Risk**: None additional.

---

## Out-of-scope (deliberately)

- Refactoring `version_check.rs` beyond the minimum needed for the normalization fix.
- Adding URL-escape sanitization at the render site (not needed once the public API contract is restored).
- Touching the on-disk cache schema. The fix is forward-compatible because we normalize on read regardless of what is cached (any old, un-normalized cached string that contains `-`/`+` suffix will be normalized at read time too — see Task 01 acceptance).

---

## Module / file impact

| File | Comment(s) | Change |
|------|-----------|--------|
| `crates/fdemon-app/src/version_check.rs` | 1, 2 | `parse_semver` + `check_for_newer_release` normalize tag; doc on both functions corrected; new regression test |
| `crates/fdemon-tui/src/runner.rs` | 3, 4 | Both spawn sites also check `version_check_timeout_secs > 0` |
| `crates/fdemon-app/src/config/types.rs` *(optional, only if helper introduced)* | 3, 4 | New `pub(crate) fn should_run_version_check(&BehaviorSettings) -> bool` co-located with the struct, plus unit test |

No doc changes required in `docs/CONFIGURATION.md` / `docs/ARCHITECTURE.md` / `README.md` — the existing docs already describe the *intended* behavior; the implementation moves to match.

---

## Tests required

- **Unit (version_check.rs)** — regression: a `tag_name` of `"1000.0.0-\x1b[31mEVIL"` returned from a `wiremock` server results in `check_for_newer_release` returning either `Some("1000.0.0")` or `None`, never a string containing `\x1b`. Assert by checking the returned string is composed only of `[0-9.]`.
- **Unit (config or runner)** — `should_run_version_check` returns `false` when `version_check = true` but `version_check_timeout_secs = 0`, and `true` when both are positive.

No integration tests, no manual-test-plan additions beyond the existing PR's checklist.

---

## Sequencing

Tasks 01 and 02 touch disjoint files and have no dependency on each other.

```
[01 version-check tag normalization & docs] ─┐
                                             ├──► merge into feat/version-check-banner
[02 runner gate timeout=0 as disabled] ──────┘
```
