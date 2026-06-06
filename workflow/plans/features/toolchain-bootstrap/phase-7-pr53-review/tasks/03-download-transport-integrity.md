## Task: Harden download transport, integrity, and archive extraction (F-PR53-03/04/05)

**Severity:** MEDIUM (security / integrity)

**Objective**: Close three download-pipeline gaps: (a) the Android cmdline-tools
download has **no checksum** and its sdkmanager/java binaries are executed
unverified; (b) `download_to_file` uses reqwest's default redirect policy (up to
10 redirects, allows `https→http` downgrade and cross-host) with no scheme check;
(c) `extract_tar_xz` silently **skips** path-traversal/symlink-escape entries
instead of failing closed (unlike `extract_zip`), and its doc references a
non-existent "channel-based guard".

**Depends on**: — (chain B start; shares files with tasks 04 and 05)

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/download.rs`
- `crates/fdemon-daemon/src/toolchain/android_install.rs`

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs` (`verify_sha256` usage at 914-925 as the reference pattern)
- `crates/fdemon-daemon/src/toolchain/types.rs` (cmdline-tools URL / sha override field)

### Details

**(a) Android cmdline-tools — no integrity check.**
`install_android_tools_inner` (`android_install.rs:256-409`) calls
`download_to_file` (line 260) then `spawn_blocking(extract_zip)` (line 285) with
**no `verify_sha256` between them**; the resulting `sdkmanager`/`java` are then
executed (lines 340, 387). The Flutter path verifies
(`flutter_install.rs:914-925`). `android_install.rs:20-22` documents
"No SHA verification" and that the `cmdline_tools_sha256` override "is not
implemented here". (Google publishes no fetchable per-build SHA for the floating
`_latest.zip`, so a pinned hash may be infeasible — in that case transport
hardening in (b) is the primary mitigation, plus honoring any configured override.)

**(b) `download_to_file` redirect/scheme.**
`download.rs:288-293` builds the reqwest client with only
`user_agent`/`connect_timeout`/`read_timeout` and `.build()` — no `.redirect(...)`
and no scheme check on `url`. The module doc (download.rs:171-176) leans on the
HTTPS guarantee that an unconstrained plaintext redirect silently undermines.
URLs derive from the (HTTPS-fetched) release manifest / config.

**(c) `extract_tar_xz` silent-skip.**
`download.rs:703-747` relies solely on `tar::Archive::unpack`, which silently
skips unsafe entries (the test at 1199-1224 asserts extraction *succeeds* while
the traversal entry is dropped). `extract_zip` (line 565) calls
`sanitize_entry_path` and fails closed. The doc comment at 696-697 references a
"channel-based guard" that does not exist (the mpsc channel is a byte pipeline only).

### Proposed Fix

1. **Transport**: in `download_to_file`, reject non-`https://` URLs up front, and
   install a custom redirect policy that errors on a non-https hop, bounds the
   redirect count (e.g. ≤5), and re-validates the final URL scheme.
2. **Android integrity**: if a `cmdline_tools_sha256` override is configured, call
   `verify_sha256` (via `spawn_blocking`) on the downloaded zip **before**
   `extract_zip`, mirroring the Flutter path; wire the override through. If no
   hash is available, rely on (1)'s HTTPS/redirect guarantee and document the
   residual corruption-detection gap in the module doc.
3. **tar.xz fail-closed**: iterate entries explicitly
   (`tar::Archive::entries()`), run `sanitize_entry_path` on each `entry.path()`,
   and return `Err` on any traversal/absolute/symlink-escape entry (matching
   `extract_zip`). Correct the doc comment to remove the non-existent
   "channel-based guard" reference.

### Acceptance Criteria

1. `download_to_file` returns an error for a non-https URL and for a redirect that
   downgrades to http or exceeds the redirect bound; an all-https redirect chain
   within the bound still succeeds.
2. When a cmdline-tools SHA override is present, a mismatching download is rejected
   **before** any extraction/execution; matching passes.
3. `extract_tar_xz` returns `Err` on an archive containing a `../` traversal or an
   escaping symlink entry (no longer silently succeeds); a clean archive extracts
   unchanged. The test that previously asserted silent-skip is updated to assert
   the error.
4. The `extract_tar_xz` doc no longer references a non-existent channel-based guard.

### Testing

```rust
// download.rs test module
// - scheme/redirect: unit-test the URL-scheme guard; (if feasible) a mock server
//   test that a 30x to http:// is rejected. At minimum test the pure scheme check.
// - tar.xz: build an archive with a "../escape" entry and an escaping symlink;
//   assert extract_tar_xz returns Err and writes nothing outside dest_dir.
//   (Replace test_extract_tar_xz_rejects_traversal's "silently skipped" assertion.)
// android_install.rs test module
// - verify_sha256-before-extract: with a configured override and a tampered zip,
//   assert install fails before extract_zip is reached.
```

### Notes

- Writes `download.rs` (shared with task 04) and `android_install.rs` (shared with
  task 05) — task 03 runs first in chain B; 04 and 05 follow and are mutually
  file-disjoint (parallelizable after 03).
- Severity is medium (defense-in-depth + corruption robustness), not RCE: URLs are
  HTTPS to official CDNs and the Flutter path already hashes. Do not over-engineer
  a pinned-hash scheme for the floating Android `_latest.zip` if upstream provides none.
