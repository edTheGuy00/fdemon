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
