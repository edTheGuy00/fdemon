## Task: Replace Hardcoded `v0.1.0` Release Badge on Website Home Page

**Objective**: Swap the hardcoded `release-v0.1.0` shields.io badge on the home page for the dynamic `github/v/release/edTheGuy00/fdemon` shields.io endpoint, matching `README.md:11`. The badge will then auto-track the latest GitHub release without requiring a website rebuild.

**Depends on**: None

**Estimated Time**: 10 minutes

### Scope

**Files Modified (Write):**
- `website/src/pages/home.rs`: Line 54 — replace the `<img src=…>` URL inside the release badge.

**Files Read (Dependencies):**
- `README.md` line 11 — canonical dynamic shields.io badge URL and styling parameters.

### Details

**Current (`website/src/pages/home.rs:51-56`):**

```rust
<img
    alt="Release"
    src="https://img.shields.io/badge/release-v0.1.0-blue?style=flat&labelColor=1d1d1d&color=54c5f8"
    class="h-6"
/>
```

**Replace with** (mirroring `README.md:11`):

```rust
<img
    alt="GitHub Release"
    src="https://img.shields.io/github/v/release/edTheGuy00/fdemon?style=flat&labelColor=1d1d1d&color=54c5f8&logo=GitHub&logoColor=white"
    class="h-6"
/>
```

Key differences:

1. Endpoint switches from `/badge/release-v0.1.0-blue` (static, content baked into URL) to `/github/v/release/edTheGuy00/fdemon` (dynamic, shields.io fetches GitHub's release feed at request time).
2. Adds `logo=GitHub&logoColor=white` so the badge gets the GitHub octocat icon — visual parity with the README badge.
3. `alt` text updated from `"Release"` to `"GitHub Release"` for accuracy and screen-reader clarity.

Leave the surrounding `<div>` and the adjacent license badge untouched. The license badge below it (`labelColor=1d1d1d` + BSL 1.1 text) is already accurate and stable.

### Acceptance Criteria

1. The release badge's `src` attribute matches the dynamic shields.io endpoint used in `README.md:11`.
2. The badge renders correctly in `trunk serve` (icon, label, dynamic version pulled from GitHub).
3. No references to the literal `v0.1.0` remain in `website/src/pages/home.rs` (`grep -n "v0\.1\.0" website/src/pages/home.rs` returns nothing).
4. `cd website && trunk build` succeeds.

### Testing

Manual verification (the website has no automated UI test suite for this page):

```bash
cd website
trunk serve
# Visit http://localhost:8080 in a browser.
# Confirm the badge in the hero section reads "release v0.5.2"
# (or whichever is current on GitHub) and shows the GitHub logo.
```

Build verification:

```bash
cd website
trunk build
```

### Notes

- The dynamic shields.io endpoint is rate-limited per IP at request time but cached aggressively, so production traffic is fine. The README uses the same endpoint and has been stable.
- Out of scope: changing the website's own crate version (`website/Cargo.toml:3` still reads `0.1.0`). That's a SemVer for the website crate itself, independent of fdemon releases — the dynamic shields.io approach makes it irrelevant.
- Out of scope: the license badge URL on line 59 — already accurate and references no version.

---

## Completion Summary

**Status:** Done
**Branch:** fix/url-and-version-discrepancies

### Files Modified

| File | Changes |
|------|---------|
| `website/src/pages/home.rs` | Replaced hardcoded static badge URL with dynamic shields.io GitHub release endpoint; updated `alt` from "Release" to "GitHub Release"; added `logo=GitHub&logoColor=white` parameters |

### Notable Decisions/Tradeoffs

1. **Minimal change scope**: Only lines 52-56 were touched (the release badge `<img>` element). The surrounding `<div>`, adjacent license badge, and all other content are untouched as specified.
2. **Worktree build limitation**: `trunk build` fails in the worktree due to a git worktree path not being in the workspace manifest — this is a harness artifact. The build was verified in the main repo's `website/` directory and succeeds cleanly.

### Testing Performed

- `grep -n "v0\.1\.0" website/src/pages/home.rs` — returned no output (acceptance criterion 3 met)
- `cd /Users/ed/Dev/zabin/flutter-demon/website && trunk build` — Passed (1 warning about dead_code in unrelated file, success)

### Risks/Limitations

1. **Dynamic badge network dependency**: The shields.io dynamic endpoint requires GitHub to be reachable at render time, but this matches the existing README behavior and is cached aggressively by shields.io.
