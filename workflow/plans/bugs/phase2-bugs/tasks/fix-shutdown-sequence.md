## Task: Fix Shutdown Sequence (Bug #3)

**Objective**: Modify the Flutter process shutdown sequence to properly stop the running Flutter app before disconnecting the daemon protocol, ensuring both Flutter Demon and the Flutter app terminate when the user quits.

**Depends on**: `fix-response-routing` (Bug #2 must be fixed first for reliable command responses)

### Problem Summary

Currently, pressing 'q' to quit Flutter Demon sends `daemon.shutdown` which only disconnects the daemon protocol layer. The Flutter app itself continues running in the simulator/device because `app.stop` is never sent.

### Scope

- `src/daemon/process.rs`: Modify `shutdown()` method signature and implementation
- `src/tui/mod.rs`: Update shutdown call to pass required parameters

### Implementation Steps

1. **Modify `FlutterProcess::shutdown()` signature**
   
   Change from:
   ```rust
   pub async fn shutdown(&mut self) -> Result<()>
   ```
   
   To:
   ```rust
   pub async fn shutdown(
       &mut self, 
       app_id: Option<&str>, 
       cmd_sender: Option<&CommandSender>
   ) -> Result<()>
   ```

2. **Update shutdown implementation in `process.rs`**
   
   ```rust
   pub async fn shutdown(
       &mut self,
       app_id: Option<&str>,
       cmd_sender: Option<&CommandSender>,
   ) -> Result<()> {
       use std::time::Duration;
       use tokio::time::timeout;

       info!("Initiating Flutter process shutdown");

       // Step 1: Stop the app if we have an app_id and command sender
       if let (Some(id), Some(sender)) = (app_id, cmd_sender) {
           info!("Stopping Flutter app: {}", id);
           match sender.send_with_timeout(
               DaemonCommand::Stop { app_id: id.to_string() },
               Duration::from_secs(5)
           ).await {
               Ok(_) => info!("App stop command acknowledged"),
               Err(e) => warn!("App stop command failed (continuing): {}", e),
           }
       }

       // Step 2: Send daemon.shutdown command
       let shutdown_cmd = r#"{"method":"daemon.shutdown","id":9999}"#;
       let _ = self.send_json(shutdown_cmd).await;

       // Step 3: Wait up to 5 seconds for graceful exit
       match timeout(Duration::from_secs(5), self.child.wait()).await {
           Ok(Ok(status)) => {
               info!("Flutter process exited gracefully: {:?}", status);
               Ok(())
           }
           Ok(Err(e)) => {
               warn!("Error waiting for process: {}", e);
               self.force_kill().await
           }
           Err(_) => {
               warn!("Timeout waiting for graceful exit");
               self.force_kill().await
           }
       }
   }
   ```

3. **Update `run_with_project()` in `tui/mod.rs`**
   
   Change the shutdown call from:
   ```rust
   if let Err(e) = p.shutdown().await {
   ```
   
   To:
   ```rust
   if let Err(e) = p.shutdown(
       state.current_app_id.as_deref(),
       cmd_sender.as_ref()
   ).await {
   ```

4. **Add necessary import in `process.rs`**
   
   ```rust
   use super::commands::{CommandSender, DaemonCommand, RequestTracker};
   ```

### Acceptance Criteria

1. Pressing 'q' or Ctrl+C stops both Flutter Demon AND the Flutter app
2. The app on the simulator/device is no longer running after quit
3. No orphan Flutter processes remain after Flutter Demon exits
4. Graceful shutdown completes within 10 seconds (5s for app.stop + 5s for daemon.shutdown)
5. If app.stop fails, shutdown still proceeds to daemon.shutdown and force kill

### Testing

1. **Manual Test - Normal Quit**
   - Start Flutter Demon with a Flutter project
   - Wait for app to launch on simulator/device
   - Press 'q' to quit
   - Verify: Flutter Demon closes
   - Verify: Flutter app on device/simulator stops
   - Verify: `ps aux | grep flutter` shows no orphan processes

2. **Manual Test - Ctrl+C**
   - Repeat above with Ctrl+C instead of 'q'

3. **Manual Test - App Already Stopped**
   - Start Flutter Demon
   - Manually stop the app from the device
   - Press 'q' in Flutter Demon
   - Verify: Shutdown completes without errors (app.stop may fail, but that's OK)

4. **Unit Test Considerations**
   - Test that shutdown() with None app_id skips the app.stop step
   - Test that shutdown() with None cmd_sender skips the app.stop step
   - Test that app.stop failure doesn't prevent daemon.shutdown

### Notes

- The `app.stop` command may timeout if Bug #2 isn't fully fixed, but since we catch errors, shutdown will still proceed
- Force kill ensures cleanup even if graceful shutdown fails
- `kill_on_drop(true)` on the child process provides an additional safety net

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/daemon/process.rs` - Modified `shutdown()` signature and implementation
- `src/tui/mod.rs` - Updated shutdown call to pass `app_id` and `cmd_sender`

**Implementation Details:**
- Added `DaemonCommand` import to `process.rs`
- Changed `shutdown()` signature to accept `app_id: Option<&str>` and `cmd_sender: Option<&CommandSender>`
- Added Step 1 in shutdown: Send `app.stop` command if both `app_id` and `cmd_sender` are available
  - Uses 5-second timeout for app.stop
  - Logs success or failure but continues to daemon.shutdown either way
- Cloned `cmd_sender` before passing to `run_loop()` so it remains available for shutdown

**Notable Decisions:**
- Used `cmd_sender.clone()` to preserve ownership for shutdown - acceptable since `CommandSender` is cheap to clone (just channel + Arc)
- App.stop failures are logged but don't prevent shutdown - ensures robust cleanup

**Testing Performed:**
- `cargo check` - PASS
- `cargo test` - PASS (218 tests)
- `cargo clippy` - PASS (no warnings)

**Risks/Limitations:**
- Manual testing against a real Flutter project is recommended to verify end-to-end functionality
- The 5-second timeout for app.stop + 5-second timeout for daemon.shutdown = up to 10 seconds total shutdown time