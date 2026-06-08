# Task 04a — Document the version-keyed cache + platform paths

**Agent:** implementor
**Depends on:** 01, 02, 03
**Estimated:** 0.5h

## Objective

Update user-facing docs to describe the version-check cache behavior, its platform-specific
location, and how to clear it.

## Files (Write)

- `docs/CONFIGURATION.md`

## Steps

1. In the `[behavior] version_check` section of `docs/CONFIGURATION.md`, add a short subsection
   "Version-check cache":
   - Results are cached for 24h at `dirs::cache_dir()/fdemon/version_check.json`.
   - The cache is keyed to the binary version that wrote it; a cache written by a different
     fdemon version is ignored (prevents a dev build from suppressing a release build's banner).
   - Platform paths:
     - Linux: `~/.cache/fdemon/version_check.json` (or `$XDG_CACHE_HOME/fdemon/...`)
     - macOS: `~/Library/Caches/fdemon/version_check.json`
     - Windows: `%LOCALAPPDATA%\fdemon\version_check.json`
   - To force an immediate re-check, delete that file. Setting `[behavior] version_check = false`
     disables the check entirely.
2. Note the banner now also appears on the main screen for auto-launch users (not only above the
   New Session Dialog) and dismisses on the first keypress.

## Acceptance criteria

- [ ] `docs/CONFIGURATION.md` documents cache location (all 3 platforms), 24h TTL, version-keying,
      and how to clear it.
- [ ] Mentions the expanded banner scope + dismiss-on-keypress behavior.

## Out of scope

- Do NOT edit `docs/ARCHITECTURE.md` (Task 04b / doc_maintainer owns it).

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-af0252afa482594f2

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Added "Version-check cache" subsection under `#### version_check_timeout_secs`, documenting cache location (all 3 platforms), 24h TTL, version-keyed format, atomic writes, how to clear, and expanded banner scope + dismiss-on-keypress behavior. |

### Notable Decisions/Tradeoffs

1. **Subsection placement**: Added as `#### Version-check cache` directly after the `version_check_timeout_secs` subsection, before `### Watcher Settings`. This keeps all version-check material together within the Behavior Settings section without disrupting the heading hierarchy.
2. **Cache format documented from source**: Field names (`checked_at`, `current_version`, `latest`) were taken directly from the `CacheEntry` struct in `version_check.rs` to ensure accuracy.
3. **XDG note**: Added clarification for Linux that `$XDG_CACHE_HOME` overrides `~/.cache` since the code uses `dirs::cache_dir()` which respects XDG.

### Testing Performed

- Visual inspection of rendered Markdown structure — section fits naturally between `version_check_timeout_secs` and `### Watcher Settings`.
- All acceptance criteria checked:
  - [x] Documents cache location for all 3 platforms
  - [x] Documents 24h TTL
  - [x] Documents version-keying (cross-version poisoning prevention)
  - [x] Documents how to clear the cache (per-platform delete commands)
  - [x] Mentions expanded banner scope (New Session Dialog + main screen)
  - [x] Mentions dismiss-on-keypress behavior

### Risks/Limitations

1. **Windows path**: Documented as `%LOCALAPPDATA%\fdemon\version_check.json` based on `dirs::cache_dir()` behavior on Windows. If `dirs` uses a different path on Windows, the doc should be updated accordingly.
