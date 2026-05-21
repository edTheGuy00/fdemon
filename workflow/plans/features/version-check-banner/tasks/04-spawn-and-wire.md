## Task: Spawn the version check and wire it into the TUI runner

**Objective**: Add `spawn_version_check` to `fdemon-app::spawn` following the exact pattern of `spawn_tool_availability_check`, and invoke it from the TUI runner during startup — gated on `settings.behavior.version_check`.

**Depends on**: 01-version-check-module, 02-config-key, 03-banner-refactor

**Agent:** implementor

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**

- `crates/fdemon-app/src/spawn.rs`: Add `pub fn spawn_version_check(msg_tx: mpsc::Sender<Message>)` mirroring `spawn_tool_availability_check` at lines 356-374.

- `crates/fdemon-tui/src/runner.rs`:
  - At line 77 (inside `run_with_project`) and line 199 (inside `run_with_project_and_dap`), after the existing `spawn::spawn_tool_availability_check(engine.msg_sender());`, add a guarded call:

    ```rust
    if engine.settings().behavior.version_check {
        spawn::spawn_version_check(engine.msg_sender());
    }
    ```

  - Confirm `engine.settings()` is the correct accessor — adapt if the binding name differs in this file.

**Files Read (Dependencies):**

- `crates/fdemon-app/src/version_check.rs` (from task 01): consumes `check_for_newer_release`.
- `crates/fdemon-app/src/message.rs` (from task 03): consumes `Message::NewVersionAvailable`.
- `crates/fdemon-app/src/config/types.rs` (from task 02): consumes `BehaviorSettings::version_check`.

### Details

**`spawn_version_check`** — copy the shape of `spawn_tool_availability_check` directly:

```rust
/// Spawn the GitHub release version check in the background.
///
/// Silent: any failure (network, parse, version-not-newer) drops the
/// task without sending a Message — no banner is rendered in that case.
/// The check is fire-and-forget and never blocks startup.
pub fn spawn_version_check(msg_tx: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        if let Some(latest) = crate::version_check::check_for_newer_release().await {
            let _ = msg_tx
                .send(Message::NewVersionAvailable { latest })
                .await;
        }
    });
}
```

No explicit timeout wrapper here — `check_for_newer_release` already configures `reqwest::Client` with a 3-second timeout. Adding another `tokio::time::timeout` would be belt-and-suspenders without value.

**Runner wiring** — both entry points (`run_with_project` and `run_with_project_and_dap`) need the same call:

```rust
spawn::spawn_tool_availability_check(engine.msg_sender());
if engine.settings().behavior.version_check {
    spawn::spawn_version_check(engine.msg_sender());
}
```

The `if` guard ensures `version_check = false` users emit no outbound HTTP at all — the `tokio::spawn` is never reached.

**Why not gate inside `spawn_version_check`**: keeps the `spawn::` API surface uniform (all `spawn_*` functions unconditionally spawn) and makes the opt-out auditable at the call site.

**Headless explicitly excluded**: `src/headless/runner.rs` does **not** get a `spawn_version_check` call. Confirmed during planning — CI/scripted runs should not generate stderr chatter, and there is no banner surface in headless mode anyway.

### Acceptance Criteria

1. `cargo build --workspace` succeeds.
2. `cargo test -p fdemon-app spawn` passes (existing tests + any new one for `spawn_version_check` — see below).
3. Running `fdemon` in a project directory (manually) shows the version banner if a newer GitHub release exists (mock via temporarily setting `version = "0.0.1"` in `Cargo.toml` to force "newer remote" — revert before committing).
4. Setting `[behavior] version_check = false` in `.fdemon/config.toml` results in **no** outbound HTTP request (verify with `tcpdump` / `lsof` / `strace`, or by observing that no `tracing::debug!` from `version_check.rs` fires).
5. Headless mode (`fdemon --headless …`) does not perform a version check — `grep` confirms `spawn_version_check` is only referenced in `crates/fdemon-tui/src/runner.rs`.

### Testing

Spawn-function unit test — verify the channel send happens when the inner function returns `Some`:

```rust
#[tokio::test]
async fn spawn_version_check_sends_message_on_some() {
    // We can't easily mock check_for_newer_release without restructuring,
    // so this test instead verifies the message shape directly:
    let (tx, mut rx) = mpsc::channel::<Message>(1);
    tx.send(Message::NewVersionAvailable { latest: "0.6.0".into() })
        .await
        .unwrap();

    let msg = rx.recv().await.unwrap();
    assert!(matches!(
        msg,
        Message::NewVersionAvailable { latest } if latest == "0.6.0"
    ));
}
```

(The real value of `spawn_version_check` is hard to unit-test without hitting the network or restructuring `check_for_newer_release` to take an injected fetcher. Defer that to a future test-infrastructure task; for now, manual smoke testing per the acceptance criteria above is sufficient.)

### Notes

- The `tokio::spawn` orphans the task on shutdown — that's fine for a 3-second fire-and-forget check. The runner does not need to track the JoinHandle.
- This task is the integration point — once it lands, end-to-end behavior should be observable in `cargo run`.
- Manual smoke checklist (run before marking done):
  1. `cargo run` from a Flutter project root → confirm New Session Dialog has no banner if `tag_name` matches current version.
  2. Edit workspace `Cargo.toml` to `version = "0.0.1"` → `cargo run` → confirm banner appears with correct format.
  3. Revert `Cargo.toml`.
  4. Add `[behavior]\nversion_check = false` to `.fdemon/config.toml` → `cargo run` with `Cargo.toml` still at `0.0.1` → confirm no banner.
  5. Drop network (e.g. wifi off) → `cargo run` → confirm no banner, no error UI, dialog renders normally.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/version-check-banner

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale>

### Testing Performed

- `cargo build --workspace` — Pending
- `cargo test --workspace` — Pending
- Manual smoke (5 scenarios above) — Pending

### Risks/Limitations

1. **<Risk>**: <Description>
