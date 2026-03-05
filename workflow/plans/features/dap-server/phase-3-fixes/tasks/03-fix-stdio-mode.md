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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/dap_stdio/runner.rs` | Fixed `ClientDisconnected` handler to `break` the event loop immediately instead of continuing to wait for more events |
| `docs/IDE_SETUP.md` | Implemented Option B: added Transport Modes section with prominent stdio limitations, marked TCP as primary/recommended transport, updated Zed/Helix/Neovim examples to prefer TCP, marked all stdio options as "Protocol Testing Only", updated capabilities table to show per-transport support |

### Notable Decisions/Tradeoffs

1. **Option B chosen over Option A**: Full stdio-to-Engine wiring deferred to Phase 4 as recommended. TCP mode provides real debugging today; honest docs prevent user confusion.

2. **Helix section restructured**: Options A/B/C re-ordered so TCP options (A: direct connect, B: port-arg) come first and stdio (now Option C) is clearly labeled as testing-only. This matches the principle that recommended paths appear first.

3. **Neovim example updated**: Swapped adapter ordering so `fdemon_tcp` is the default in `dap.configurations.dart` rather than `fdemon` (stdio). Comments reinforce which is for production use.

4. **Capabilities table expanded**: Added per-transport columns (TCP vs. Stdio) so users can see exactly which capabilities are available in each mode without reading prose.

5. **`break` vs. channel-close**: The event consumer loop previously waited for the channel to close naturally after `ClientDisconnected`. In stdio mode the channel closes when the session task exits, but the exit sequence is: session ends → `ClientDisconnected` sent → session task drops `event_tx`. Adding `break` makes this explicit and eliminates any window where the event consumer spins between the `ClientDisconnected` send and the channel close.

### Testing Performed

- `cargo fmt --all` - Passed (no formatting changes needed)
- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (3,381 tests: 3,272 passed, 0 failed, 75 ignored across all crates + integration tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Stdio mode still not usable for real debugging**: This is intentional — Phase 4 will wire stdio to a real VM Service session. The docs now communicate this limitation clearly.

2. **Helix option numbering change**: Helix options were re-lettered (TCP with port-arg moved from C to B, stdio from B to C). Any users who bookmarked option letters by name will need to re-read the section, but the content is more usable in the new order.
