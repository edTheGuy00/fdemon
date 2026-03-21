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
