# Bug: Version-check banner never appears

## Summary

The GitHub version-check banner (`⬆ New version available: vX.Y.Z (current vA.B.C)`) never
surfaces, even when the running binary is genuinely behind the latest GitHub release.
Root-caused via a multi-agent research workflow (2026-06-08). The banner is blocked by
**three independent defects**; one is live and primary, two are latent.

## Reproduction (observed)

- Running `fdemon 0.5.6`; latest GitHub release is `v0.5.7` (`releases/latest` returns
  `{"tag_name":"v0.5.7","draft":false,"prerelease":false}`, HTTP 200 — data source healthy).
- No banner appears on startup.
- Live cache file on disk: `~/.cache/fdemon/version_check.json → {"checked_at":1780888475,"latest":null}`,
  ~8h old, inside the 24h TTL.

## Root cause

### Defect #1 — Poisoned, version-blind 24h disk cache (LIVE, primary)

`crates/fdemon-app/src/version_check.rs`

- The cache stores the **comparison _result_** (`latest: null`) rather than the raw fetched
  tag (`:277-280`).
- The cache file path is **not keyed by binary version** (`cache_path()` at `:58-59`), and
  `CacheEntry` has no `current_version` field (`:48-54`).
- Consequence: a locally-built `0.5.7` binary (the workspace is now `version = "0.5.7"` in
  `Cargo.toml:7`) computes `0.5.7 > 0.5.7 == false`, writes `{"latest":null}`, and **poisons
  the shared cache**. Any other binary run within 24h — including the released `0.5.6`
  artifact — hits the fresh-cache branch (`:244-258`), executes `None.and_then(...)` (`:250`),
  short-circuits to `None` **without any network request**, and shows no banner.

> NOTE: The original feature plan (`workflow/plans/features/version-check-banner/PLAN.md`
> Decision 3) explicitly specified **no persisted cache** ("per-launch only"). The 24h cache
> was added afterward as a "hardening follow-up" (PR #49) and is the direct cause of this bug.
> Per user decision (2026-06-08), we **keep** the cache but fix it, rather than removing it.

### Defect #2 — Detector is blind in its own version (LATENT)

`version_check.rs:240, :270`

`env!("CARGO_PKG_VERSION")` is compile-time. A build from the current `0.5.7` tree compares
`0.5.7 > 0.5.7 == false` via the strict `>`, never fires, and **re-poisons the cache** every
time the entry expires. Any build whose version equals the latest release is permanently silent.

### Defect #3 — Notice dropped outside the New Session Dialog (LATENT)

`crates/fdemon-app/src/handler/update.rs:384-394`, `crates/fdemon-app/src/state.rs:1755-1756`,
`crates/fdemon-tui/src/render/mod.rs:262-279`

- The handler drops `Message::NewVersionAvailable` unless `is_new_session_dialog_visible()`
  (which matches only `UiMode::NewSessionDialog | Startup`).
- Even if the notice were stored, the banner is **rendered only inside the New Session Dialog
  widget** (`render/mod.rs:270`), so there is no render site in `Loading` / `Normal` /
  `InstallWizard` modes.
- Consequence: auto-launch users (`auto_start` / `behavior.auto_launch` + cached device) go
  `Startup → Loading → Normal` without ever showing the dialog, so they never see the banner —
  deterministically, not a race.

## Evidence chain (verified)

| Stage | Verdict | Proof |
|---|---|---|
| Gate `should_run_version_check()` | PASS | defaults `version_check=true`, timeout=3s; cache file exists → task ran |
| Spawn `spawn_version_check` | PASS | `spawn.rs:361-371`, sends `NewVersionAvailable` only on `Some` |
| HTTP `fetch_latest_tag` | PASS (bypassed) | UA + Accept + 3s + rustls; GitHub 200 live; not reached on cache hit |
| **Cache read** | **FAIL** | `version_check.rs:244-258` fresh `latest:null` → short-circuit `None` |
| Parse `parse_semver` | PASS | `:267`, strips `v` at `:206-210` |
| **Compare `latest > current`** | **FAIL (self-blind)** | `:270` `(0,5,7) > (0,5,7)` false → writes `latest:null` |
| Message send | FAIL (consequence) | `spawn.rs` sends only on `Some` |
| **Handler/state gate** | **FAIL** | `update.rs:384-394` drops notice unless dialog visible |
| Render | PASS (unreached) | `render/mod.rs:262-279` only renders banner inside the dialog |

## Fix design

### Decision A — Cache the raw tag, re-compare at read time, version-key the entry (Defect #1)

`CacheEntry` becomes:

```rust
pub(crate) struct CacheEntry {
    pub checked_at: u64,
    pub current_version: String, // env!("CARGO_PKG_VERSION") at write time
    pub latest: Option<String>,  // raw fetched tag (bare semver), independent of `current`
}
```

- **Store the raw fetched tag** (`Some(normalized_latest)`), not the filtered comparison
  `result`. The read path already re-compares (`:250-257`), so storing the raw tag makes that
  re-comparison meaningful for whatever binary reads it.
- **Validate `current_version` on read**: if `entry.current_version != env!("CARGO_PKG_VERSION")`,
  treat the entry as stale (ignore it, fetch fresh). This prevents a `0.5.7` build's cache from
  ever masking a `0.5.6` binary.
- On read, when fresh AND version matches: re-parse the cached raw tag and compare against
  `current` (existing logic at `:250-257`, unchanged).
- Migration: an old-format cache (missing `current_version`) fails `serde` deserialization →
  `read_cache_at` returns `None` → treated as a cache miss → fetches fresh. No explicit migration
  code needed; document it.

### Decision B — Keep strict `>`, but never persist a self-blind "no update" as authoritative (Defect #2)

The strict `>` is correct: a binary should not advertise its own version as an update. We do **not**
loosen the comparison. Defect #2's real harm was *cache poisoning*, which Decision A eliminates
(the raw tag `0.5.7` is now stored, and a `0.5.6` reader re-derives `0.5.7 > 0.5.6 == true`).
What remains is intrinsic and acceptable: a binary equal to the latest release shows no banner.
- Add a `tracing::debug!` when `latest == current` to make this explicit in logs.
- (No code behavior change beyond Decision A; this decision is mostly documentation + the debug line.)

### Decision C — Decouple the banner from the New Session Dialog (Defect #3)

Two coordinated changes:

1. **Handler**: store `state.startup_notice = Some(...)` unconditionally (remove the
   `is_new_session_dialog_visible()` gate in `update.rs:384-394`). Keep a `debug!` trace.
2. **Render**: add a standalone one-line banner render site so the notice surfaces outside the
   dialog. Render the banner as the **top row of the main screen** whenever `startup_notice`
   is `Some` and the dialog is not already rendering it (`Loading` and `Normal` modes). Reuse
   the existing `render_startup_notice` formatting from the dialog widget (extract it to a
   shared location, e.g. `render/mod.rs` or a small helper, so both the dialog and the main
   screen render identical copy).
   - **Lifecycle (decided O1): dismiss on first keypress.** The notice is stored per-launch and
     rendered on the main/loading screen until the user's first interaction, then cleared. It is
     also already cleared by `hide_new_session_dialog()` (`state.rs:1698`). Implementation: clear
     `state.startup_notice` on the first key event handled while `ui_mode` is not the dialog
     (e.g. in the key-handling entry point, set `startup_notice = None` once). Keep it simple —
     a single "clear on any key in Normal/Loading" is sufficient; no timer wiring.

### Decision D — Out of scope

- No change to `[behavior] version_check` opt-out, spawn wiring, HTTP client, or endpoint.
- No removal of the cache (user opted to keep it, fixed).

## File-level change list

### Modify

| File | Change | Defect |
|---|---|---|
| `crates/fdemon-app/src/version_check.rs` | Add `current_version` to `CacheEntry`; store raw tag not `result` (`:277-280`); validate `current_version` + freshness on read (`:244-258`); debug-log `latest==current` | #1, #2 |
| `crates/fdemon-app/src/handler/update.rs` | Remove `is_new_session_dialog_visible()` gate; always set `startup_notice` (`:384-394`) | #3 |
| `crates/fdemon-tui/src/render/mod.rs` | Add top-row banner render for `Loading`/`Normal` modes when `startup_notice.is_some()`; share formatting with dialog | #3 |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Expose/extract `render_startup_notice` formatting for reuse (no copy divergence) | #3 |
| `docs/CONFIGURATION.md` (or `docs/ARCHITECTURE.md`) | Document the version-keyed cache format + 24h TTL + non-dialog banner scope | docs |

### Tests (in-crate)

| File | New/updated tests |
|---|---|
| `crates/fdemon-app/src/version_check.rs` | `CacheEntry` round-trips with `current_version`; cache write stores raw tag (not `result`); read ignores entry whose `current_version != CARGO_PKG_VERSION`; read serves banner when cached raw tag is newer than a *different* (older) current; old-format cache → treated as miss |
| `crates/fdemon-app/src/handler/update.rs` (or state tests) | `NewVersionAvailable` sets `startup_notice` even when `ui_mode == Loading`/`Normal` |
| `crates/fdemon-tui/src/render/mod.rs` / dialog widget | banner renders on `Normal`/`Loading` when notice present; no double-render when dialog is visible |

## Acceptance criteria

1. On a `0.5.6` binary with latest GitHub `v0.5.7`, the banner appears — **even if** a `0.5.7`
   build previously wrote the cache (no cross-version poisoning).
2. Deleting the cache and launching shows the banner (already true; must remain true).
3. Auto-launch users (no New Session Dialog) see the banner on the main screen.
4. A binary whose version equals the latest release shows no banner (correct), and does not
   write a cache entry that would suppress an older binary.
5. `cargo test --workspace` green; `cargo clippy --workspace` clean; `cargo fmt --all` applied.

## Resolved decisions (2026-06-08)

- **O1 — Non-dialog banner lifecycle → DISMISS ON FIRST KEYPRESS.** See Decision C.
- **O2 — Immediate unblock → DONE.** The poisoned cache on the Linux dev box
  (`/home/ed/.cache/fdemon/version_check.json`) was deleted. See platform note below.
- **O3 — Cache keying → `current_version` FIELD (single file).** See Decision A.

### Platform cache-path note

`dirs::cache_dir()` is platform-specific. The poisoned file was found/deleted on Linux at
`~/.cache/fdemon/version_check.json`. On **macOS** the cache lives at
`~/Library/Caches/fdemon/version_check.json` (NOT `~/.cache`). On Windows it is
`%LOCALAPPDATA%\fdemon\version_check.json`. Any manual cache-clearing guidance in docs must
name the platform-correct path.

## Next step

On approval of the high-level approach (and answers to O1–O3), I will produce `TASKS.md` +
individual task files with a File Overlap Analysis. Provisional task split:

1. `version_check.rs` cache rework + tests (Defect #1/#2) — fdemon-app only.
2. Handler gate removal + state test (Defect #3a) — fdemon-app only.
3. Render decoupling + dialog formatting extraction + render tests (Defect #3b) — fdemon-tui only.
4. Docs update.

(Tasks 1 and 2 share `fdemon-app` but different files → parallel-safe; Task 3 is fdemon-tui →
parallel-safe; Task 4 docs → after 1–3.)
