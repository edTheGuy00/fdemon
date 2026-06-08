# Code Review: Version-Check Banner Not Appearing

**Review Date:** 2026-06-08
**Change Type:** Bug Fix (3 independent defects)
**Diff Base:** `1d21c89..HEAD` (commits `a74834f` → `f6b15ab`)
**Branch:** `fix/version-check-banner-not-appearing`

## Overall Verdict: ⚠️ APPROVED WITH CONCERNS

The fix correctly and completely addresses all three documented root causes. No critical
or major issues, no architecture violations, no security vulnerabilities, no logic errors.
The single ⚠️ is an architectural consistency nit (direct field assignment vs. a named
state method). All other findings are minor/nitpick: stale rustdoc comments, a magic-number
constant, and a few test-coverage gaps. **Safe to merge as-is**; the concerns below are
worth a short follow-up but none block.

## Agent Verdicts

| Agent | Verdict | Blocking? |
|-------|---------|-----------|
| `bug_fix_reviewer` | ✅ APPROVED | No |
| `architecture_enforcer` | ⚠️ WARNING (1) | No |
| `code_quality_inspector` | ✅ APPROVED (minor/nitpick only) | No |
| `logic_reasoning_checker` | ✅ PASS | No |
| `security_reviewer` | ✅ PASS (2 medium, 2 low) | No |

## Root-Cause Verification

All three defects from `BUG.md` are correctly diagnosed and fixed (confirmed by
bug_fix_reviewer + logic_reasoning_checker independently tracing the paths):

- **Defect #1 — poisoned version-blind cache:** `CacheEntry` now carries `current_version`;
  the write path always stores the **raw fetched tag** (`Some(normalized)`), never the
  filtered `latest: null` result; the read path gates a cache hit on
  `fresh && current_version == CARGO_PKG_VERSION`, so a `0.5.7`-written entry is ignored by
  a `0.5.6` reader, which then fetches fresh and shows the banner. ✅
- **Defect #2 — self-blind comparison:** Strict `>` correctly preserved; the real harm
  (cache poisoning) is eliminated by Defect #1's raw-tag storage; a `debug!` documents the
  `latest == current` case. ✅
- **Defect #3 — notice dropped/unrendered outside the dialog:** Handler stores
  `startup_notice` unconditionally; a standalone top-row banner renders on `Normal`/`Loading`
  (and all non-dialog modes) via a shared `startup_notice_line` formatter, with the content
  area shrunk by one row; dismiss-on-first-keypress is wired. ✅

Logic reviewer verified all 5 BUG.md acceptance criteria are logically met. Regression risk
rated **Low** — changes are additive and well-isolated; existing snapshot tests
(`startup_notice = None`) are unaffected.

## Consolidated Findings

### ⚠️ Concern (architecture consistency) — non-blocking

**C1. Direct field assignment for the `startup_notice` set path**
[Source: architecture_enforcer]
`crates/fdemon-app/src/handler/update.rs:~390` writes
`state.startup_notice = Some(StartupNotice::NewVersionAvailable { latest })` directly. Every
other `AppState` lifecycle mutation routes through a named method, and *both* clear paths
(`hide_new_session_dialog`, `dismiss_startup_notice_on_interaction`) already do. The set path
is the lone exception.
→ **Suggested:** add `AppState::set_startup_notice(&mut self, notice: StartupNotice)` and call
it from the handler. Behaviour-neutral; closes the encapsulation gap.

### 🟡 Minor — documentation freshness (in-code rustdoc)

**C2. Stale `Message::NewVersionAvailable` doc** [Source: code_quality_inspector]
`crates/fdemon-app/src/message.rs:~692` still says the notice is only "so the New Session
Dialog renders a one-line banner." Post-fix it also appears on Normal/Loading. Update to
describe the full lifecycle.

**C3. Incomplete `startup_notice` field doc** [Source: code_quality_inspector]
`crates/fdemon-app/src/state.rs:~1441` documents only the `hide_new_session_dialog` clear
path; add the second path (`dismiss_startup_notice_on_interaction`, first keypress in a
non-dialog mode).

### 🟡 Minor — test-coverage gaps

**C4. No `UiMode::Startup` dismiss no-op test** [Source: bug_fix_reviewer, code_quality_inspector]
`is_new_session_dialog_visible()` returns true for both `NewSessionDialog` **and** `Startup`,
but only the former has a no-op dismiss test. Add a `Startup`-mode variant so a future refactor
that compares `ui_mode` directly can't silently regress.

**C5. No "terminal too short" banner-guard test** [Source: code_quality_inspector]
The `area.height < BANNER_MIN_HEIGHT` guard in `render/mod.rs` is untested. Add a 1-row-render
test asserting no banner text and no zero-height layout rect.

**C6. No redirect-rejection test** [Source: security_reviewer]
`reqwest::redirect::Policy::none()` is set but never exercised. Add a wiremock test returning
301/302 and asserting `fetch_latest_tag` → `None`, so the defense can't regress silently.

### 🔵 Nitpick — maintainability

**C7. Magic `1` in banner layout** [Source: code_quality_inspector, bug_fix_reviewer]
`render/mod.rs:~202` uses bare `1` literals for the banner row height / `area.y + 1` /
`area.height - 1`. Per CODE_STANDARDS Responsive-Layout Principle 4, introduce
`const BANNER_ROW_HEIGHT: u16 = 1;` (and a derivation comment on `BANNER_MIN_HEIGHT = 2`).

**C8. Test name misnomer** [Source: code_quality_inspector, bug_fix_reviewer]
`write_stores_raw_tag_not_result` drives `fetch_latest_tag` + `write_cache_at` directly rather
than `check_for_newer_release`. Rename to `cache_always_stores_raw_tag_on_successful_fetch`,
or add an end-to-end `check_for_newer_release` wiremock test that reads the cache back.

### Security — medium/low (defense-in-depth; user-owned file, not exploitable)

**S1 (MEDIUM). Cache read has no size cap** [Source: security_reviewer]
`read_cache_at` does `std::fs::read` + `serde_json::from_slice` with no pre-check; the network
path caps at 512 KiB but the cache path does not. Add a `metadata().len()` guard (~1 MiB) before
read for symmetry. Threat is limited — the file is user-owned.

**S2 (MEDIUM). Predictable `.tmp` sibling on atomic write** [Source: security_reviewer]
`path.with_extension("tmp")` is a fixed name. Single-writer (one fire-and-forget spawn/process)
makes the concurrent-write race non-existent in practice; a UUID-suffixed temp name would close
the theoretical gap. Low priority.

**S3 (LOW). `latest` not validated at the `Message` boundary** [Source: security_reviewer]
The only producer (`spawn.rs`) always passes a normalized digit-and-dot string, so no raw remote
bytes reach the ratatui renderer today. A `debug_assert!` (or a validating constructor) at the
handler would harden the public TEA boundary against future misuse. Defense-in-depth only.

## Documentation Freshness (Phase 3.5)

✅ **No stale project docs.** No new modules/crates, no `Cargo.toml`/build-step changes. The
diff already updates `docs/ARCHITECTURE.md` (version-keyed cache + decoupled render path) and
`docs/CONFIGURATION.md` (cache format, platform paths, TTL, banner scope) as part of the fix.
A factual inaccuracy in the ARCHITECTURE.md update (a non-existent late-arrival drop gate) was
caught in orchestration and corrected in commit `f6b15ab`. The only remaining doc nits (C2, C3)
are in-code rustdoc, captured above.

## Recommendation

**Merge approved.** Optionally land a small follow-up addressing C1 (named setter), C2–C3
(rustdoc), and S1 (cache size cap) — these are the highest-value, lowest-effort items. The
test-coverage gaps (C4–C6) and nitpicks (C7–C8) can be batched into the same follow-up or
deferred. No item blocks this merge.
