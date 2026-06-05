## Task: Make download tests deterministic & exercise the real manifest path (F2, F13)

**Severity:** HIGH (F2 — flaky acceptance test) + MEDIUM (F13)

**Objective**: Turn the two key Phase 5 download tests into real, deterministic guards:
the mid-stream cancellation test must not pass/fail by timing, and the manifest
404/malformed tests must drive the actual `fetch_release_manifest` function instead of
re-implementing its logic inline.

**Depends on**: 04 (same files: `download.rs`, `flutter_install.rs`)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/download.rs` (cancel test)
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs` (manifest tests + small refactor)

### Details & Fixes

**F2 (HIGH) — `cancel_mid_stream_returns_cancelled_and_cleans_part` is flaky.**
The test (`download.rs:1636-1701`) serves a 200 KiB body, waits for the first progress
callback via `Notify`, then cancels and asserts `expect_err` is `Cancelled`. On
loopback the whole 200 KiB frequently arrives as a **single** chunk: the one progress
callback fires, the test cancels, but the loop's next `stream.next()` returns `None`
(exhausted), so it breaks via the **success** path, renames the `.part`, and returns
`Ok(())` → `expect_err` panics at `:1693`. Reproduced: 1-in-5 to 2-in-15 failures.
The `Notify` only guarantees *one* chunk landed, not that more remain. This is the test
that proves abort acceptance criteria #1/#2, so a flaky pass undermines the whole
feature's verification.
**Fix (make cancellation deterministic — do NOT weaken the assertion):** keep
`expect_err` + `is_cancelled()`, but force the stream to remain in-flight at cancel
time. Either:
- (a) Have the progress callback itself `token.cancel()` on first invocation, and serve
  a body that keeps feeding (throttled/large enough) so the next biased `select!`
  iteration observes the already-set cancellation and returns `Error::cancelled` — the
  cancel is set synchronously before the next `stream.next()` await, so the biased
  branch wins deterministically; or
- (b) Serve a multi-chunk body with a server-side inter-chunk delay (e.g. a small
  axum/hyper test server yielding chunks with `tokio::time::sleep`, or wiremock
  `ResponseTemplate::set_delay` + a body large enough to guarantee ≥2 chunk
  iterations) so a `stream.next()` is reliably pending when the token is cancelled.

Option (a) is self-contained (no chunking dependence) and preferred.

**F13 (MEDIUM) — manifest error tests replicate logic instead of calling the fn.**
`fetch_manifest_404_is_clear_error` (`:1633`) and
`fetch_manifest_malformed_json_is_clear_error` (`:1686`) mount wiremock mocks but
**never call** `fetch_release_manifest` — because `manifest_url()` (`:248-251`)
hard-codes the GCS URL with no override, the tests copy the production status-check +
parse logic inline (the bodies even comment *"Replicate the logic of
fetch_release_manifest"*) and re-create the two error strings verbatim. The real
function's HEAD probe (`check_network_connectivity`, `:351`), `is_success()` branch
(`359-364`), and `RawManifest → FlutterReleaseManifest` mapping (`366-389`) are never
exercised; the mounted HEAD mocks are dead. Reordering or changing the error strings
in the real function would leave these tests green.
**Fix (make the real path reachable):** prefer URL parameterisation — keep
`pub async fn fetch_release_manifest(platform)` as a thin wrapper that computes
`manifest_url(&platform)` and delegates to a new
`fetch_release_manifest_from(url: &str)`; point the wiremock tests (404, malformed, and
the HEAD-probe case) at `fetch_release_manifest_from(mock_url)` so the HEAD→GET→parse
sequencing and field mapping are exercised end-to-end. (Alternative: extract an
`async fn parse_manifest_response(resp, url)` helper owning the `is_success()` check,
parse, and field mapping, and call it from both the function and the tests.) Prefer the
URL-parameterisation variant since it also validates the HEAD probe.

### Acceptance Criteria

1. The mid-stream cancellation test passes deterministically across repeated runs
   (e.g. `cargo test -p fdemon-daemon --lib cancel_mid_stream` 20× with no failures),
   still asserting `Err` is `Cancelled` and no `.part` remains (F2).
2. The manifest 404 and malformed-JSON tests call the real `fetch_release_manifest`
   (via `fetch_release_manifest_from` or the shared parse helper) against a wiremock
   server, so changing the production error strings or HEAD/GET/parse ordering would
   fail a test (F13).
3. The HEAD-probe path in the real function is exercised by at least one test.

### Testing

```bash
# Determinism check for F2
for i in $(seq 1 20); do cargo test -p fdemon-daemon --lib cancel_mid_stream -- --exact || break; done
```

```rust
// flutter_install.rs test module (F13)
// - 404: mock GET returns 404 -> fetch_release_manifest_from(mock_url) is Err with the
//        real "manifest HTTP {status} for {url}" message.
// - malformed: mock GET returns invalid JSON -> Err with the real
//        "failed to parse manifest from {url}: {e}" message.
// - happy: mock HEAD 200 + GET valid manifest -> Ok with correctly mapped fields,
//        proving the HEAD->GET->parse path and RawManifest->FlutterReleaseManifest map.
```

### Notes

- Do not weaken F2's assertion to accept `Ok` with no `.part` — that no longer proves
  "cancelling mid-stream returns the cancellation error promptly."
- Shares files with Task 04 — serialise (chain B). The F5 `read_timeout` change from
  Task 04 does not affect these tests (the cancel loop polls `stream.next()`
  independently of client timeout config).
