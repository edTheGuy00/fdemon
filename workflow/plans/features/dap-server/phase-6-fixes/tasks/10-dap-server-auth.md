## Task: DAP Server Authentication Token and Idle Timeout

**Objective**: Add a startup-generated authentication token to the DAP server and an idle timeout for established sessions, addressing the two security findings M1 and L9.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-dap/src/server/mod.rs`: Token generation, logging, and connection-level validation
- `crates/fdemon-dap/src/server/session.rs`: Token validation in `initialize`, idle timeout
- `crates/fdemon-dap/src/protocol/types.rs`: Add `authToken` field to `InitializeArguments`

**Files Read (Dependencies):**
- None

### Details

#### Part 1: M1 — Auth Token

**Step 1:** Generate a cryptographically random token at server startup in `DapServer::start()`:

```rust
use rand::Rng;

fn generate_auth_token() -> String {
    let bytes: [u8; 16] = rand::thread_rng().gen();
    hex::encode(bytes) // 32-char hex string
}
```

If `rand` is not already a dependency, use `getrandom` or `std` (Rust 1.80+ has `std::random`). Alternatively, use UUID v4.

**Step 2:** Log the token at startup so the IDE can retrieve it:

```rust
tracing::info!("DAP server listening on {}:{}", bind_addr, port);
tracing::info!("DAP auth token: {}", token);
// Also print to stderr for IDE integration:
eprintln!("DAP auth token: {}", token);
```

**Step 3:** Store the token in the server and pass it to each session.

**Step 4:** In `handle_initialize`, validate the token:

```rust
// In InitializeArguments (protocol/types.rs), add:
#[serde(rename = "authToken")]
pub auth_token: Option<String>,

// In session.rs handle_initialize:
if let Some(expected) = &self.auth_token {
    let provided = args.auth_token.as_deref().unwrap_or("");
    if provided != expected.as_str() {
        return vec![self.make_response(
            DapResponse::error(request, "Authentication failed: invalid or missing auth token")
        )];
    }
}
```

**Step 5:** Make auth opt-in initially: add a `require_auth: bool` field to `DapServerConfig` (default `false`). When `true`, the token is required. When `false`, any client can connect (preserving backward compatibility). Log a warning when auth is disabled.

#### Part 2: L9 — Idle Timeout

**Step 1:** Add an idle timeout constant:

```rust
const IDLE_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
```

**Step 2:** Track last activity timestamp in the session:

```rust
let mut last_activity = tokio::time::Instant::now();
```

Update `last_activity` on every received message.

**Step 3:** Add an idle check arm to the `tokio::select!` loop in `run_inner`:

```rust
_ = tokio::time::sleep_until(last_activity + IDLE_TIMEOUT),
    if self.state != SessionState::Attached => {
    tracing::warn!("DAP session idle for {:?}, closing", IDLE_TIMEOUT);
    break;
}
```

The `if self.state != SessionState::Attached` guard ensures that active debug sessions (which may legitimately be idle while the debuggee runs) are not disconnected. The timeout only applies to sessions stuck in `Initializing` or `Configured` states.

### Acceptance Criteria

1. Server generates a random auth token at startup and logs it
2. When `require_auth` is enabled, `initialize` without a valid token returns an error response
3. When `require_auth` is disabled (default), any client can connect (backward compatible)
4. Sessions in non-`Attached` state are disconnected after 5 minutes of inactivity
5. Active `Attached` sessions are not affected by the idle timeout
6. Existing tests pass: `cargo test -p fdemon-dap`
7. `cargo clippy -p fdemon-dap` clean

### Testing

```rust
#[tokio::test]
async fn test_initialize_with_valid_token_succeeds() {
    // Create session with auth required, provide correct token
    // Assert initialize succeeds
}

#[tokio::test]
async fn test_initialize_with_invalid_token_rejected() {
    // Create session with auth required, provide wrong token
    // Assert error response
}

#[tokio::test]
async fn test_initialize_without_token_rejected_when_required() {
    // Create session with auth required, omit token
    // Assert error response
}

#[tokio::test]
async fn test_idle_timeout_disconnects_non_attached_session() {
    // Create session, send initialize, then wait
    // Assert session closes after IDLE_TIMEOUT
}

#[tokio::test]
async fn test_attached_session_not_affected_by_idle_timeout() {
    // Create fully attached session, wait beyond IDLE_TIMEOUT
    // Assert session remains open
}
```

### Notes

- Auth is opt-in initially to avoid breaking existing workflows. A future release can make it default-on.
- The `authToken` field uses `Option<String>` for backward compatibility with older clients that don't send it.
- Consider also returning the auth token in the server's startup output (e.g., as part of a JSON status line) so that IDE plugins can parse it automatically.
- The idle timeout of 5 minutes is generous. It only catches truly abandoned sessions, not slow clients.

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-dap/Cargo.toml` | Added `rand.workspace = true` dependency; added `time` feature to dev tokio |
| `crates/fdemon-dap/src/protocol/types.rs` | Added `auth_token: Option<String>` field to `InitializeRequestArguments` |
| `crates/fdemon-dap/src/server/mod.rs` | Added `require_auth: bool` to `DapServerConfig`; added `auth_token: Option<String>` to `DapServerHandle`; added `generate_auth_token()` function; updated `start()` to generate/log/print token when `require_auth=true`; updated `accept_loop` to pass token to sessions; added `auth_token()` accessor; updated all tests |
| `crates/fdemon-dap/src/server/session.rs` | Added `IDLE_TIMEOUT` constant; added `auth_token` field to `DapClientSession`; updated `new()`, `with_backend()`, `run_on_with_backend()`, `run_on()`, `run()` to accept auth token; updated `handle_initialize()` to validate token; added idle timeout arm in `run_inner()`; added 6 new auth/idle timeout tests |
| `crates/fdemon-dap/src/service.rs` | Updated `DapServerConfig` constructions to include `require_auth: false`; updated `DapServerHandle` construction to include `auth_token: None` in `start_stdio()` |
| `crates/fdemon-dap/src/transport/stdio.rs` | Updated `DapClientSession::run_on()` calls to pass `None` auth token (stdio mode needs no auth) |

### Notable Decisions/Tradeoffs

1. **Auth opt-in**: `require_auth` defaults to `false` to preserve backward compatibility. Existing users and IDE configs are unaffected. Auth must be explicitly enabled.

2. **Idle timeout guards**: The idle timeout only fires for `Initializing` and `Configured` states. `Attached` sessions (actively debugging) and `Uninitialized` sessions (covered by the existing `INIT_TIMEOUT`) are excluded. `Disconnecting` state is also excluded to avoid races.

3. **Token generation without `hex` crate**: Used manual `write!(s, "{:02x}", b)` hex encoding instead of the `hex` crate to avoid adding another dependency. `rand` was already in workspace dependencies.

4. **`eprintln!` for token**: The task spec allows this for IDE integration. In stdio mode, no token is generated (auth is not applicable), so the `eprintln!` never fires in stdio mode where stdout is the DAP pipe.

5. **Test design for idle timeout**: Used `tokio::test(start_paused = true)` with `tokio::time::advance()` to test the 300-second idle timeout without actually waiting.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-dap` - Passed
- `cargo test -p fdemon-dap` - Passed (587 tests, 0 failed)
- `cargo clippy -p fdemon-dap -- -D warnings` - Passed (0 warnings)
- `cargo check --workspace` - Passed

### Risks/Limitations

1. **`eprintln!` use**: The code standards say to never use `eprintln!`, but the task spec explicitly requires it for IDE integration (printing the auth token to stderr so IDE plugins can parse it). The `eprintln!` is conditional on `require_auth = true` which defaults to `false`, so existing workflows are unaffected.

2. **Token not persisted**: The auth token is ephemeral — regenerated on each server start. IDE plugins must parse it fresh from stderr output on each launch.
