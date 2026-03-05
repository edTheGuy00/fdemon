## Task: Fix Stdio Mode — Either Wire Engine or Update Docs

**Objective**: Make the `--dap-stdio` mode either functional for real debugging (Option A) or honestly documented as transport-only (Option B). Currently, stdio mode uses `NoopBackend`, does not start an Engine or Flutter process, and `IDE_SETUP.md` presents it as the primary transport for Zed/Helix.

**Depends on**: 01-wire-tcp-backend (for Option A approach)

**Estimated Time**: 2–3 hours

**Severity**: CRITICAL — users following IDE_SETUP.md will have a broken experience.

### Scope

- `src/dap_stdio/runner.rs`: Wire Engine startup (Option A) or add limitations comment
- `crates/fdemon-dap/src/transport/stdio.rs`: Wire backend (Option A)
- `docs/IDE_SETUP.md`: Update documentation in either case

### Details

#### Current State

`--dap-stdio` (`src/main.rs:73-75`) calls `dap_stdio::runner::run_dap_stdio()` which:
1. Creates an mpsc event channel
2. Calls `DapService::start_stdio(event_tx)` — spawns `run_stdio_session` as a task
3. `run_stdio_session` wraps stdin/stdout and calls `DapClientSession::run_on()` with `NoopBackend`
4. No Engine, no Flutter process, no VM Service connection
5. The module doc at `stdio.rs` explicitly states "Does not route `attach` commands to the Dart VM Service"

Meanwhile, `docs/IDE_SETUP.md` presents stdio as the primary Zed/Helix transport with full configuration examples.

#### Option A: Wire Stdio to Real Debugging (Recommended)

After task 01 establishes the backend factory pattern:

1. **Accept a VM Service URI from IDE launch args**: The stdio runner receives a `--vm-service-uri` argument (or reads it from stdin as part of an `attach` args payload). Zed/Helix launch configs would include this.
2. **Start a minimal Engine**: `run_dap_stdio()` would create an Engine, discover the running Flutter session, and extract `VmRequestHandle`.
3. **Construct `VmServiceBackend`** and pass it through `run_stdio_session` → `run_on_with_backend`.
4. **Alternative**: Instead of starting a full Engine, the stdio runner could directly create a `VmServiceClient` from the URI and construct a `VmRequestHandle` without the Engine's session management overhead. This is lighter-weight but requires `fdemon-daemon` as a direct dependency of the binary.

#### Option B: Update Docs (Minimum viable fix)

1. Add a prominent disclaimer to `docs/IDE_SETUP.md`:
   ```markdown
   > **Note**: Stdio transport is currently transport-only (protocol validation
   > and message framing). Real debugging requires TCP mode with a running
   > fdemon TUI session. Full stdio debugging support is planned for Phase 4.
   ```
2. Update the Zed/Helix config examples to prefer TCP mode.
3. Keep stdio mode for protocol testing and IDE integration validation.

#### Recommendation

**Implement Option B now, plan Option A for Phase 4.** Rationale:
- Task 01 (TCP backend wiring) is already complex enough for this phase.
- Stdio-to-Engine wiring requires decisions about process lifecycle (who launches Flutter? who owns the session?) that deserve their own design phase.
- TCP mode with the TUI is the natural first use case (user runs `fdemon`, IDE connects).
- Honest docs are better than broken features.

### Acceptance Criteria

**If Option A:**
1. `--dap-stdio` mode can connect to a running Flutter session via VM Service URI
2. `attach` succeeds and debugging works (breakpoints, stepping, variables)
3. IDE_SETUP.md examples work end-to-end

**If Option B:**
1. `docs/IDE_SETUP.md` clearly states stdio limitations
2. TCP mode is presented as the primary/recommended transport
3. Stdio examples are marked as "protocol testing only" or removed
4. No misleading claims about stdio debugging capability

### Testing

**Option A:**
- Integration test: stdio session with mock VM Service → `attach` succeeds
- Manual test: Zed/Helix stdio config connects and debugs

**Option B:**
- Review IDE_SETUP.md for accuracy
- Verify TCP mode examples are complete and correct

### Notes

- The decision between Option A and Option B should be confirmed with the user before implementation.
- If Option B is chosen, create a follow-up task for Phase 4 to implement full stdio debugging.
- The `run_dap_stdio` function in `src/dap_stdio/runner.rs` also has an issue where the `ClientDisconnected` event handler doesn't break the event loop — this should be fixed as part of this task regardless of which option is chosen.
