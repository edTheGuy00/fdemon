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

**Status:** Not Started
**Branch:** feat/toolchain-bootstrap
