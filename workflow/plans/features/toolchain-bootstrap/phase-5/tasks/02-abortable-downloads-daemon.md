## Task: Abortable downloads — daemon-side cancellation

**Objective**: Thread a `CancellationToken` through `download_to_file` and the two
install orchestrators so an in-flight download/extract can be cancelled cleanly,
with no orphaned `.part` file left behind.

**Depends on**: 01-disk-network-preflight (shares the download trio of files)

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/toolchain/download.rs`: add a `CancellationToken`
  parameter to `download_to_file`; race the streaming chunk loop against
  `token.cancelled()` with `tokio::select!`; add a `.part`-cleanup Drop guard;
  return a distinct cancellation error.
- `crates/fdemon-daemon/src/toolchain/flutter_install.rs`: accept + forward the
  token into `download_to_file`; check the token at attempt boundaries.
- `crates/fdemon-daemon/src/toolchain/android_install.rs`: same forwarding.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/error.rs`: add/confirm a `Cancelled` error variant (or a
  `process`-level sentinel the app can recognize).
- `crates/fdemon-daemon/src/toolchain/process_stream.rs`: pattern for the XZ decode
  thread / channel teardown.

### Details

**Cancellation primitive — `tokio_util::sync::CancellationToken`.** Research:
`AbortHandle::abort()` is too blunt (drops mid-await, not cancel-safe for I/O);
`CancellationToken` is ergonomic and shareable. `tokio_util` is already a transitive
dep via `tokio`; add it explicitly if not already a direct dep.

```rust
use tokio_util::sync::CancellationToken;

pub async fn download_to_file(
    url: &str,
    dest: &Path,
    cancel: CancellationToken,           // NEW
    mut on_progress: impl FnMut(DownloadProgress),
) -> Result<()> {
    let _part_guard = PartFileGuard::new(part_path.clone()); // Drop removes .part on early return
    // ... inside the streaming loop:
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                return Err(Error::cancelled("download cancelled"));
            }
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => { writer.write_all(&bytes)?; /* progress */ }
                    Some(Err(e)) => return Err(/* retryable */),
                    None => break,
                }
            }
        }
    }
    // success: disarm the guard, fsync, rename .part -> dest
}
```

- **Drop guard:** because the app spawns this in a `tokio::spawn` whose handle may be
  aborted (task 03), the natural end-of-function `.part` cleanup can be skipped. A
  `PartFileGuard { path, armed: bool }` whose `Drop` best-effort-removes the `.part`
  file (unless disarmed on success) guarantees abort-safe cleanup.
- **XZ decode thread:** the `extract_tar_xz` decode runs on a `std::thread`, not a
  tokio task, so it can't be aborted via `JoinHandle::abort()`. When the receiver
  (`ReceiverReader`) is dropped, the `SenderWriter` gets `BrokenPipe` on the next
  write and the decode thread terminates. **Verify** the `lzma-rs`/xz write loop
  surfaces `BrokenPipe` rather than swallowing it, and document the finding. If the
  loop can hang, add a token check in the `SenderWriter::write` path.
- **Error variant:** prefer a real `Error::Cancelled` (or `Error::cancelled(msg)`)
  so the app layer can distinguish "user cancelled" from "install failed" and avoid
  showing a scary failure message. Classify it **recoverable**, not fatal.

### Acceptance Criteria

1. `download_to_file` takes a `CancellationToken`; cancelling it mid-stream returns
   the cancellation error promptly (within one chunk iteration).
2. After a cancelled download, **no `.part` file remains** on disk (Drop guard
   verified by test).
3. `install_flutter` / `install_android_tools` accept and forward the token; both
   are no-ops-safe if cancelled before any I/O.
4. The XZ-decode teardown-on-cancel behavior is verified and documented (a comment
   in `download.rs` explaining the `BrokenPipe` termination path).
5. A distinct `Cancelled` error is returned (not a generic process error), classified
   recoverable.

### Testing

```rust
#[tokio::test]
async fn cancel_mid_stream_returns_cancelled_and_cleans_part() {
    // wiremock: serve a slow/large body; cancel the token after first chunk;
    // assert Err is Cancelled and the .part file does not exist.
}

#[tokio::test]
async fn precancelled_token_does_no_io() {
    let token = CancellationToken::new();
    token.cancel();
    // assert download_to_file returns Cancelled without creating .part
}
```

### Notes

- Keep the public signature change localized; update all existing call sites
  (preflight uses `fetch_release_manifest`, not `download_to_file`, so the blast
  radius is the two installers + their tests).
- The app-side wiring (storing the `JoinHandle`, the cancel message, the `Esc` key)
  is **task 03** — this task only delivers the daemon API.
- Resumable downloads (Range header) remain **out of scope** (deferred); abort
  simply tears the download down.

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-bootstrap

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/error.rs` | Added `Error::Cancelled` variant, `cancelled()` constructor, `is_cancelled()` predicate; added to `is_recoverable()`; 3 new tests |
| `Cargo.toml` | Added `tokio-util = { version = "0.7", features = ["rt"] }` to workspace deps |
| `crates/fdemon-daemon/Cargo.toml` | Added `tokio-util.workspace = true` |
| `crates/fdemon-app/Cargo.toml` | Added `tokio-util.workspace = true` |
| `crates/fdemon-daemon/src/toolchain/download.rs` | Added `PartFileGuard` RAII struct; rewrote `download_to_file` with `CancellationToken` param, pre-cancel check, `tokio::select!` streaming loop; updated module doc with Cancellation and XZ Decode Thread Teardown sections; updated all existing tests to pass `CancellationToken::new()`; added 4 new cancellation/guard tests |
| `crates/fdemon-daemon/src/toolchain/flutter_install.rs` | Added `CancellationToken` param to `install_flutter`, `install_inner`, `archive_install`; pre-cancel check; forwarded token to `download_to_file`; updated 2 existing tests; added 1 new pre-cancel test |
| `crates/fdemon-daemon/src/toolchain/android_install.rs` | Added `CancellationToken` param to `install_android_tools` and `install_android_tools_inner`; pre-cancel check; forwarded token to `download_to_file`; added 1 new pre-cancel test |
| `crates/fdemon-app/src/actions/mod.rs` | Updated 2 `install_flutter` / `install_android_tools` call sites to pass `CancellationToken::new()` with forward-compat comment for task 03 |

### Notable Decisions/Tradeoffs

1. **`PartFileGuard` is armed on entry, disarmed before rename**: This guarantees abort-safe cleanup even when the outer `JoinHandle` is aborted mid-await — the `Drop` impl runs on the finalizer. Disarming before the rename (not after) avoids a TOCTOU window.

2. **Pre-cancel check before disk-space preflight**: The pre-cancel check fires before `ensure_disk_space`, so a cancelled token returns immediately without any filesystem I/O. This also handles the `precancelled_token_does_no_io` acceptance criterion without special-casing the `part_path`.

3. **XZ decode thread documentation (no code change needed)**: Verified that `SenderWriter::write` returns `BrokenPipe` when the receiver is dropped, and that `lzma_rs::xz_decompress` propagates write errors immediately — so the thread terminates on the very next write after cancellation drops the `ReceiverReader`. Documented in the module `//!` header. No additional token check in `SenderWriter` is warranted.

4. **`tokio-util` features = ["rt"]**: The `rt` feature is required for `CancellationToken` to work correctly with `tokio::select!`. Using `tokio-util` as a direct dep rather than relying on transitive exposure is cleaner and future-proof.

5. **`fdemon-app` call sites use `CancellationToken::new()`**: Task 03 is responsible for storing the `JoinHandle`, wiring the cancel `Message`, and binding Esc. This task intentionally delivers only the daemon-side API, with a no-cancel placeholder at the call sites.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (all crates, including new cancellation tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **`PartFileGuard` on rename failure**: On the rename error path, we manually remove the `.part` file and propagate the error rather than re-arming the guard — the guard has already been disarmed at that point. This is a clean invariant (disarm is irreversible) but slightly subtle; documented with a comment.

2. **`cancel_mid_stream_returns_cancelled_and_cleans_part` test timing**: The test uses a 200 KiB body and cancels after the first chunk notification. On very fast networks/loopback this test could in theory complete before cancellation fires; in practice the mock server on loopback always has multiple chunks. The `Notify`-based synchronisation ensures at least one chunk has landed before cancellation is requested.
