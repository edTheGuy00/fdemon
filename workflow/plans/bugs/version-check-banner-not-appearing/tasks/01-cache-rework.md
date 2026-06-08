# Task 01 — Cache rework (store raw tag, version-key the entry)

**Agent:** implementor
**Depends on:** —
**Estimated:** 2–3h
**Fixes:** Defect #1 (poisoned, version-blind cache) + neutralizes Defect #2's harm

## Objective

Make the on-disk version-check cache immune to cross-version poisoning. Store the **raw fetched
tag** instead of the comparison result, record the **binary version** that wrote the entry, and
**ignore** any entry whose recorded version differs from the running binary's version.

## Files (Write)

- `crates/fdemon-app/src/version_check.rs`

## Background

Today (`version_check.rs`):
- `CacheEntry { checked_at, latest }` where `latest` is the **filtered result** (`None` when
  `current == latest`) — `:48-54`, `:277-280`.
- `check_for_newer_release` short-circuits on a fresh cache (`:244-258`) using only `checked_at`
  vs TTL — no version check.

A `0.5.7` build writes `latest: null`; a `0.5.6` build then reads it fresh and shows nothing.

## Steps

1. **Extend `CacheEntry`** (`:48-54`):
   ```rust
   #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
   pub(crate) struct CacheEntry {
       pub checked_at: u64,
       pub current_version: String, // env!("CARGO_PKG_VERSION") at write time
       pub latest: Option<String>,  // RAW fetched tag (bare semver), or None only when fetch found nothing
   }
   ```
   - No `#[serde(deny_unknown_fields)]`. Because all three fields are required, an **old-format**
     file (missing `current_version`) will fail to deserialize → `read_cache_at` returns `None`
     → treated as a cache miss. This is the desired migration behavior; add a code comment saying so.

2. **Store the raw tag on write** (`:265-282`): change the cache write so `latest` is the raw
   fetched/normalized tag, independent of the comparison:
   ```rust
   let tag_str = fetch_latest_tag(GITHUB_RELEASES_LATEST, timeout).await?;
   let latest = parse_semver(&tag_str)?;
   let normalized = format!("{}.{}.{}", latest.0, latest.1, latest.2);

   write_cache(&CacheEntry {
       checked_at: now,
       current_version: env!("CARGO_PKG_VERSION").to_string(),
       latest: Some(normalized.clone()), // raw tag, NOT the filtered result
   });

   if latest > current {
       Some(normalized)
   } else {
       tracing::debug!("Version check: latest {} not newer than current {:?}", normalized, current);
       None
   }
   ```
   - Note: `latest: Some(normalized)` is always written on a successful fetch now. `latest: None`
     should only occur if you choose to cache "fetch returned an unparseable/none tag" — keep it
     simple: only write the cache after a successful parse, always with `Some(normalized)`.

3. **Validate version + freshness on read** (`:244-258`):
   ```rust
   if let Some(entry) = read_cache() {
       let fresh = now.saturating_sub(entry.checked_at) < CACHE_TTL_SECS;
       let same_binary = entry.current_version == env!("CARGO_PKG_VERSION");
       if fresh && same_binary {
           return entry.latest.and_then(|tag| {
               let parsed = parse_semver(&tag)?;
               (parsed > current).then(|| format!("{}.{}.{}", parsed.0, parsed.1, parsed.2))
           });
       }
       if fresh && !same_binary {
           tracing::debug!(
               "Version check: cache written by {} but running {}, ignoring",
               entry.current_version, env!("CARGO_PKG_VERSION")
           );
       }
       // else: expired → fall through to network fetch
   }
   ```
   - Keep the existing read-time re-comparison (it now operates on the raw tag, so it is correct
     for the running binary).

4. **Update module doc comment** (`:11-16`) to describe the version-keyed format.

## Tests (same file, `#[cfg(test)] mod tests`)

- `cache_entry_roundtrips_with_current_version` — serialize/deserialize includes `current_version`.
- `old_format_cache_is_treated_as_miss` — write a JSON blob with only `checked_at`+`latest`,
  assert `read_cache_at` returns `None`.
- `write_stores_raw_tag_not_result` — using the existing `wiremock` test harness (see existing
  tests ~`:440-620`), drive a fetch where `tag_name` is **equal** to current and assert the
  written cache `latest == Some(current)` (raw tag), NOT `None`.
- `read_ignores_entry_from_different_binary_version` — write a `CacheEntry` with
  `current_version: "0.0.1"` and a newer `latest`; assert `check_for_newer_release` does NOT serve
  it from cache (it should fetch or, if you can't fetch in test, assert the version-mismatch path
  is taken — structure the test around `read_cache_at` + the comparison helper).
- `fresh_cache_serves_banner_when_raw_tag_newer` — entry with matching `current_version`, fresh
  `checked_at`, `latest` newer than current → returns `Some`.
- Keep all existing `parse_semver` / comparator tests green.

## Acceptance criteria

- [ ] `CacheEntry` has `current_version`; cache write stores the raw tag, always `Some` on success.
- [ ] Read path ignores entries with a mismatched `current_version` (debug-logged).
- [ ] Old-format cache → cache miss (no panic, no `serde` hard error surfaced).
- [ ] New + existing unit tests pass; `cargo clippy -p fdemon-app` clean.

## Out of scope

- Do not change the TTL, endpoint, HTTP client, or `Message`/spawn wiring.
- Do not touch the strict `>` comparison semantics (a binary equal to latest correctly shows no
  banner; the only behavioral change is what gets persisted).
