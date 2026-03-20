//! # Request Handlers
//!
//! DapAdapter methods for dispatching and handling DAP protocol requests.

use crate::adapter::backend::DebugBackend;
use crate::adapter::breakpoints;
use crate::adapter::stack::build_source_from_uri;
use crate::adapter::types::{DapExceptionPauseMode, StepMode};
use crate::adapter::DapAdapter;
use crate::protocol::types::{
    AttachRequestArguments, BreakpointLocation, BreakpointLocationsArguments, ContinueArguments,
    DapBreakpoint, DapSource, DapThread, ExceptionInfoArguments, PauseArguments,
    RestartFrameArguments, SetBreakpointsArguments, SetExceptionBreakpointsArguments,
    StepArguments,
};
use crate::{DapRequest, DapResponse};

use crate::adapter::types::ERR_VM_DISCONNECTED;

impl<B: DebugBackend> DapAdapter<B> {
    /// Handle a DAP request and return the response.
    ///
    /// This is the main dispatch point for all debugging commands. The session
    /// calls this for every request that requires adapter involvement.
    /// Lifecycle requests (`initialize`, `configurationDone`) are handled by
    /// the session layer before this is called.
    pub async fn handle_request(&mut self, request: &DapRequest) -> DapResponse {
        // If the VM Service disconnected mid-session (e.g., app exited), all
        // subsequent requests return a structured error. The `disconnect` command
        // is exempt so the IDE can still cleanly close the debug session.
        if self.vm_disconnected && request.command != "disconnect" {
            return DapResponse::error_with_code(
                request,
                ERR_VM_DISCONNECTED,
                "Debug session ended: VM Service disconnected",
            );
        }

        match request.command.as_str() {
            "attach" => self.handle_attach(request).await,
            "disconnect" => self.handle_disconnect(request).await,
            "threads" => self.handle_threads(request).await,
            "setBreakpoints" => self.handle_set_breakpoints(request).await,
            "setExceptionBreakpoints" => self.handle_set_exception_breakpoints(request).await,
            "continue" => self.handle_continue(request).await,
            "next" => self.handle_next(request).await,
            "stepIn" => self.handle_step_in(request).await,
            "stepOut" => self.handle_step_out(request).await,
            "pause" => self.handle_pause(request).await,
            "stackTrace" => self.handle_stack_trace(request).await,
            "scopes" => self.handle_scopes(request).await,
            "variables" => self.handle_variables(request).await,
            "evaluate" => self.handle_evaluate(request).await,
            "source" => self.handle_source(request).await,
            "loadedSources" => self.handle_loaded_sources(request).await,
            "hotReload" => self.handle_hot_reload(request).await,
            "hotRestart" => self.handle_hot_restart(request).await,
            "restartFrame" => self.handle_restart_frame(request).await,
            "callService" => self.handle_call_service(request).await,
            "exceptionInfo" => self.handle_exception_info(request).await,
            "updateDebugOptions" => self.handle_update_debug_options(request).await,
            "breakpointLocations" => self.handle_breakpoint_locations(request).await,
            _ => DapResponse::error(request, format!("unsupported command: {}", request.command)),
        }
    }

    /// Handle the `attach` request.
    ///
    /// Parses the attach arguments, calls `get_vm()` on the backend to
    /// discover existing isolates, populates the thread map, and emits a
    /// `thread` started event for each isolate found.
    ///
    /// On success, emits the following Flutter/Dart custom DAP events:
    ///
    /// - `dart.debuggerUris` — the VM Service WebSocket URI for supplementary
    ///   tooling (VS Code DevTools, etc.)
    /// - `flutter.appStart` — device ID, build mode, and restart capability
    pub(super) async fn handle_attach(&mut self, request: &DapRequest) -> DapResponse {
        let args: AttachRequestArguments = match request.arguments.as_ref() {
            Some(v) => serde_json::from_value(v.clone()).unwrap_or_default(),
            None => AttachRequestArguments::default(),
        };

        // Apply settings from attach args before making any backend calls.
        // `evaluateGettersInDebugViews` defaults to `true` when absent, matching
        // the Dart DDS adapter's default behaviour.
        if let Some(eval_getters) = args.evaluate_getters_in_debug_views {
            self.evaluate_getters_in_debug_views = eval_getters;
        }
        // `evaluateToStringInDebugViews` defaults to `true` when absent.
        if let Some(eval_to_string) = args.evaluate_to_string_in_debug_views {
            self.evaluate_to_string_in_debug_views = eval_to_string;
        }
        // `debugSdkLibraries` defaults to `false` when absent — SDK libraries
        // are non-debuggable by default so stepping stays in app code.
        self.debug_sdk_libraries = args.debug_sdk_libraries.unwrap_or(false);
        // `debugExternalPackageLibraries` defaults to `false` when absent.
        self.debug_external_package_libraries =
            args.debug_external_package_libraries.unwrap_or(false);
        // `packageName` identifies the app's own package so its URIs are
        // always treated as debuggable regardless of the external-package flag.
        if let Some(pkg) = args.package_name {
            self.app_package_name = pkg;
        }

        match self.backend.get_vm().await {
            Ok(vm_info) => {
                // Discover pre-existing isolates from the VM object.
                if let Some(isolates) = vm_info.get("isolates").and_then(|v| v.as_array()) {
                    for isolate in isolates {
                        let id = isolate.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        let name = isolate.get("name").and_then(|v| v.as_str()).unwrap_or("");

                        if id.is_empty() {
                            continue;
                        }

                        let thread_id = self.thread_map.get_or_create(id);
                        let display_name = if name.is_empty() {
                            format!("Thread {thread_id}")
                        } else {
                            name.to_string()
                        };
                        self.thread_names.insert(thread_id, display_name);

                        let body = serde_json::json!({
                            "reason": "started",
                            "threadId": thread_id,
                        });
                        self.send_event("thread", Some(body)).await;
                    }
                }

                // ── Flutter/Dart custom events ─────────────────────────────
                //
                // Emit dart.debuggerUris with the VM Service WebSocket URI.
                // IDEs (notably VS Code's Dart extension) use this to connect
                // supplementary tooling such as DevTools.
                if let Some(uri) = self.backend.ws_uri().await {
                    tracing::debug!("Emitting dart.debuggerUris: {}", uri);
                    let body = serde_json::json!({
                        "vmServiceUri": uri,
                    });
                    self.send_event("dart.debuggerUris", Some(body)).await;
                }

                // Emit flutter.appStart with device/mode metadata.
                // supportsRestart is true for debug builds, false for profile/release.
                let device_id = self.backend.device_id().await;
                let mode = self.backend.build_mode().await;
                let supports_restart = mode == "debug";
                let app_start_body = serde_json::json!({
                    "deviceId": device_id,
                    "mode": mode,
                    "supportsRestart": supports_restart,
                });
                tracing::debug!(
                    "Emitting flutter.appStart: deviceId={:?} mode={} supportsRestart={}",
                    device_id,
                    mode,
                    supports_restart,
                );
                self.send_event("flutter.appStart", Some(app_start_body))
                    .await;

                DapResponse::success(request, None)
            }
            Err(e) => DapResponse::error(request, format!("Failed to attach: {e}")),
        }
    }

    /// Handle the `threads` request.
    ///
    /// Returns all known threads with their human-readable names. When a
    /// thread name is unavailable the fallback `"Thread N"` is used.
    pub(super) async fn handle_threads(&mut self, request: &DapRequest) -> DapResponse {
        let mut threads: Vec<DapThread> = self
            .thread_map
            .all_threads()
            .map(|(id, _isolate_id)| {
                let name = self
                    .thread_names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| format!("Thread {id}"));
                DapThread { id, name }
            })
            .collect();

        // Sort by ID for deterministic ordering.
        threads.sort_by_key(|t| t.id);

        let body = serde_json::json!({ "threads": threads });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `setBreakpoints` request.
    ///
    /// The `setBreakpoints` request is **per-file**: the client sends the
    /// complete desired set of breakpoints for one source file. This handler
    /// diffs the incoming list against the current state, removes breakpoints
    /// that are no longer wanted, and adds new ones via the VM Service backend.
    ///
    /// Breakpoints that cannot be immediately verified (e.g., no isolate is
    /// attached yet) are returned with `verified: false` and a descriptive
    /// message. The IDE will receive a `breakpoint` event (via
    /// [`handle_debug_event`] `BreakpointResolved`) when they resolve.
    ///
    /// ## Conditional Breakpoints
    ///
    /// Each [`SourceBreakpoint`] may include a `condition` (Dart expression)
    /// and/or `hit_condition` (e.g., `">= 3"`). These are stored in the
    /// [`BreakpointEntry`] and evaluated at pause time in
    /// [`handle_debug_event`]. The VM itself always sets an unconditional
    /// breakpoint; filtering is done adapter-side on each `PauseBreakpoint`
    /// event.
    ///
    /// # Source path conversion
    ///
    /// Source paths are converted to `file://` URIs. Full `package:` URI
    /// resolution (via `.dart_tool/package_config.json`) is deferred to
    /// Phase 4.
    pub(super) async fn handle_set_breakpoints(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: setBreakpoints");

        let args = match parse_args::<SetBreakpointsArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        // Convert the source path to a file:// URI for the VM Service.
        let source_path = args.source.path.as_deref().unwrap_or("");
        let uri = path_to_dart_uri(source_path);

        // Desired breakpoints from the request (empty = clear all for this source).
        let desired = args.breakpoints.unwrap_or_default();

        // ── Step 0: Record desired state (survives hot restart) ───────────────
        //
        // Store the full desired set before touching the active state so that
        // on_isolate_runnable can re-apply them after a hot restart.
        {
            let desired_bps: Vec<crate::adapter::breakpoints::DesiredBreakpoint> = desired
                .iter()
                .zip(1i64..)
                .map(|(sbp, i)| {
                    // Reuse an existing DAP ID if we already have one at this line,
                    // otherwise allocate a new one from the active state counter.
                    let existing_dap_id = self.breakpoint_state.find_by_source_line(&uri, sbp.line);
                    // Use existing DAP ID if available; otherwise use a
                    // placeholder index that will be replaced in Step 3 below.
                    let dap_id = existing_dap_id.unwrap_or(i);
                    crate::adapter::breakpoints::DesiredBreakpoint {
                        dap_id,
                        line: sbp.line as i32,
                        column: sbp.column.map(|c| c as i32),
                        condition: sbp.condition.clone(),
                        hit_condition: sbp.hit_condition.clone(),
                        log_message: sbp.log_message.clone(),
                    }
                })
                .collect();
            self.desired_breakpoints.insert(uri.clone(), desired_bps);
        }

        // ── Step 1: Remove breakpoints no longer wanted ───────────────────────

        // Snapshot existing entries for this source before mutating.
        let existing: Vec<(i64, i32, String)> = self
            .breakpoint_state
            .iter_for_uri(&uri)
            .map(|e| (e.dap_id, e.line.unwrap_or(0), e.vm_id.clone()))
            .collect();

        for (dap_id, existing_line, vm_id) in &existing {
            let still_wanted = desired.iter().any(|d| d.line as i32 == *existing_line);

            if !still_wanted {
                if let Some(isolate_id) = self.primary_isolate_id() {
                    let _ = self.backend.remove_breakpoint(&isolate_id, vm_id).await;
                }
                self.breakpoint_state.remove_by_dap_id(*dap_id);
                tracing::debug!("Removed breakpoint {} (dap_id={})", vm_id, dap_id);
            }
        }

        // ── Step 2: Add / preserve breakpoints from the desired set ──────────

        let mut response_breakpoints: Vec<DapBreakpoint> = Vec::with_capacity(desired.len());

        for sbp in &desired {
            // Reuse an existing breakpoint at this exact line.
            if let Some(dap_id) = self.breakpoint_state.find_by_source_line(&uri, sbp.line) {
                if let Some(entry) = self.breakpoint_state.lookup_by_dap_id(dap_id) {
                    response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
                }
                continue;
            }

            // New breakpoint: attempt to add via the VM Service backend.
            match self.primary_isolate_id() {
                Some(isolate_id) => {
                    match self
                        .backend
                        .add_breakpoint(
                            &isolate_id,
                            &uri,
                            sbp.line as i32,
                            sbp.column.map(|c| c as i32),
                        )
                        .await
                    {
                        Ok(result) => {
                            let actual_line = result.line.or(Some(sbp.line as i32));
                            let actual_col = result.column.or(sbp.column.map(|c| c as i32));
                            let dap_id = self.breakpoint_state.add_with_condition(
                                result.vm_id.clone(),
                                uri.clone(),
                                actual_line,
                                actual_col,
                                result.resolved,
                                breakpoints::BreakpointCondition {
                                    condition: sbp.condition.clone(),
                                    hit_condition: sbp.hit_condition.clone(),
                                    log_message: sbp.log_message.clone(),
                                },
                            );
                            tracing::debug!(
                                "Added breakpoint {}:{} → vm_id={} dap_id={} condition={:?} hit_condition={:?} log_message={:?}",
                                uri,
                                sbp.line,
                                result.vm_id,
                                dap_id,
                                sbp.condition,
                                sbp.hit_condition,
                                sbp.log_message,
                            );
                            let entry = self
                                .breakpoint_state
                                .lookup_by_dap_id(dap_id)
                                .expect("entry was just inserted");
                            response_breakpoints.push(entry_to_dap_breakpoint(entry, &args.source));
                        }
                        Err(err) => {
                            tracing::warn!(
                                "Failed to add breakpoint at {}:{}: {}",
                                uri,
                                sbp.line,
                                err
                            );
                            response_breakpoints.push(DapBreakpoint {
                                id: None,
                                verified: false,
                                message: Some(format!("Could not set breakpoint: {}", err)),
                                source: Some(args.source.clone()),
                                line: Some(sbp.line),
                                column: sbp.column,
                                ..Default::default()
                            });
                        }
                    }
                }
                None => {
                    // No isolate attached yet: return unverified pending breakpoint.
                    tracing::debug!(
                        "No active isolate; breakpoint at {}:{} is pending",
                        uri,
                        sbp.line
                    );
                    response_breakpoints.push(DapBreakpoint {
                        id: None,
                        verified: false,
                        message: Some(
                            "Breakpoint pending: no active debug session attached yet".to_string(),
                        ),
                        source: Some(args.source.clone()),
                        line: Some(sbp.line),
                        column: sbp.column,
                        ..Default::default()
                    });
                }
            }
        }

        // ── Step 3: Sync desired breakpoints with actual DAP IDs ─────────────
        //
        // After the active state is built, we have the real DAP IDs. Update the
        // desired_breakpoints entry so that re-application after hot restart
        // uses the correct stable IDs.
        {
            let synced: Vec<crate::adapter::breakpoints::DesiredBreakpoint> = desired
                .iter()
                .zip(response_breakpoints.iter())
                .filter_map(|(sbp, dap_bp)| {
                    // Only record desired breakpoints that have a DAP ID assigned.
                    dap_bp
                        .id
                        .map(|dap_id| crate::adapter::breakpoints::DesiredBreakpoint {
                            dap_id,
                            line: sbp.line as i32,
                            column: sbp.column.map(|c| c as i32),
                            condition: sbp.condition.clone(),
                            hit_condition: sbp.hit_condition.clone(),
                            log_message: sbp.log_message.clone(),
                        })
                })
                .collect();
            if synced.is_empty() {
                self.desired_breakpoints.remove(&uri);
            } else {
                self.desired_breakpoints.insert(uri.clone(), synced);
            }
        }

        let body = serde_json::json!({ "breakpoints": response_breakpoints });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `setExceptionBreakpoints` request.
    ///
    /// Maps DAP exception filter names to VM Service exception pause modes and
    /// applies the mode to all known isolates.
    ///
    /// # Supported Filters
    ///
    /// | DAP Filter   | VM Service Mode                   |
    /// |--------------|-----------------------------------|
    /// | `"All"`      | [`DapExceptionPauseMode::All`]    |
    /// | `"Unhandled"`| [`DapExceptionPauseMode::Unhandled`] |
    /// | (none)       | [`DapExceptionPauseMode::None`]   |
    ///
    /// `"All"` takes precedence when both `"All"` and `"Unhandled"` are present.
    /// Unknown filter strings produce a DAP error response.
    pub(super) async fn handle_set_exception_breakpoints(
        &mut self,
        request: &DapRequest,
    ) -> DapResponse {
        tracing::debug!("DAP adapter: setExceptionBreakpoints");

        let args = match parse_args::<SetExceptionBreakpointsArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        // Validate all filter strings before applying any.
        for filter in &args.filters {
            match filter.as_str() {
                "All" | "Unhandled" | "None" => {}
                other => {
                    tracing::warn!("Unknown exception pause mode filter: {}", other);
                    return DapResponse::error(
                        request,
                        format!("Unknown exception filter: {}", other),
                    );
                }
            }
        }

        let mode = exception_filter_to_mode(&args.filters);
        self.exception_mode = mode;

        // Apply the mode to all known isolates.
        let isolate_ids: Vec<String> = self
            .thread_map
            .all_threads()
            .map(|(_, iso)| iso.to_string())
            .collect();

        for isolate_id in &isolate_ids {
            let _ = self
                .backend
                .set_exception_pause_mode(isolate_id, mode)
                .await;
        }

        tracing::debug!(
            "Exception pause mode set to '{:?}' across {} isolate(s)",
            mode,
            isolate_ids.len()
        );

        // DAP spec: exception breakpoints response has empty breakpoints array.
        let body = serde_json::json!({ "breakpoints": [] });
        DapResponse::success(request, Some(body))
    }

    /// Return the isolate ID of the primary (first registered) isolate, if any.
    ///
    /// Used as the target for breakpoint operations when no specific isolate is
    /// requested. In a typical Flutter app there is exactly one main isolate.
    pub(super) fn primary_isolate_id(&self) -> Option<String> {
        self.thread_map
            .all_threads()
            .next()
            .map(|(_, iso)| iso.to_string())
    }

    /// Return the isolate ID of the most recently paused isolate, if any.
    ///
    /// Used by `handle_evaluate` to pick the evaluation context when no
    /// `frameId` is given. Returns `None` if no isolate is currently paused.
    pub(super) fn most_recent_paused_isolate(&self) -> Option<&str> {
        self.paused_isolates.last().map(String::as_str)
    }

    /// Handle the `continue` request.
    ///
    /// Resumes the isolate associated with the given thread ID. Invalidates all
    /// per-stop state (variable references and frame IDs) before resuming, since
    /// those references are only valid while the debuggee is stopped.
    ///
    /// Returns `allThreadsContinued: true` because Dart resumes all isolates
    /// together when a continue is issued.
    pub(super) async fn handle_continue(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: continue");

        let args = match parse_args::<ContinueArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let isolate_id = match self.thread_map.isolate_id_for(args.thread_id) {
            Some(id) => id.to_string(),
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown thread ID: {}", args.thread_id),
                )
            }
        };

        // Invalidate stopped-state references before resuming.
        self.on_resume();

        match self.backend.resume(&isolate_id, None, None).await {
            Ok(()) => {
                let body = serde_json::json!({ "allThreadsContinued": true });
                DapResponse::success(request, Some(body))
            }
            Err(e) => DapResponse::error(request, format!("Continue failed: {e}")),
        }
    }

    /// Handle the `next` (step over) request.
    ///
    /// Steps over the current statement, remaining in the same function.
    pub(super) async fn handle_next(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: next");
        self.step(request, StepMode::Over).await
    }

    /// Handle the `stepIn` request.
    ///
    /// Steps into a function call on the current line.
    pub(super) async fn handle_step_in(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: stepIn");
        self.step(request, StepMode::Into).await
    }

    /// Handle the `stepOut` request.
    ///
    /// Steps out of the current function, resuming execution after the call site.
    pub(super) async fn handle_step_out(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: stepOut");
        self.step(request, StepMode::Out).await
    }

    /// Common implementation for step operations (`next`, `stepIn`, `stepOut`).
    ///
    /// Parses `StepArguments`, resolves the thread ID to an isolate ID,
    /// invalidates per-stop state, and calls `resume` with the given step mode.
    ///
    /// The `granularity` field (if present) is ignored in Phase 3 — Dart VM
    /// only supports line-level stepping.
    pub(super) async fn step(&mut self, request: &DapRequest, mode: StepMode) -> DapResponse {
        let args = match parse_args::<StepArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let isolate_id = match self.thread_map.isolate_id_for(args.thread_id) {
            Some(id) => id.to_string(),
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown thread ID: {}", args.thread_id),
                )
            }
        };

        // Invalidate stopped-state references before resuming.
        self.on_resume();

        match self.backend.resume(&isolate_id, Some(mode), None).await {
            Ok(()) => DapResponse::success(request, None),
            Err(e) => DapResponse::error(request, format!("Step failed: {e}")),
        }
    }

    /// Handle the `pause` request.
    ///
    /// Requests the Dart VM to pause the specified isolate. The isolate will
    /// pause at the next safe point and emit a `PauseInterrupted` event, which
    /// is translated to a `stopped` DAP event with reason `"pause"`.
    pub(super) async fn handle_pause(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: pause");

        let args = match parse_args::<PauseArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let isolate_id = match self.thread_map.isolate_id_for(args.thread_id) {
            Some(id) => id.to_string(),
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown thread ID: {}", args.thread_id),
                )
            }
        };

        match self.backend.pause(&isolate_id).await {
            Ok(()) => DapResponse::success(request, None),
            Err(e) => DapResponse::error(request, format!("Pause failed: {e}")),
        }
    }

    /// Handle the `disconnect` request at the adapter level.
    ///
    /// Parses the optional `terminateDebuggee` field from the arguments:
    ///
    /// - `terminateDebuggee: true` — calls `stop_app()` on the backend to
    ///   terminate the Flutter process.
    /// - `terminateDebuggee: false` (default) — resumes any currently paused
    ///   isolates so the app continues running after the debugger detaches.
    ///   This matches the semantics of `attach` mode where the IDE merely
    ///   observes an already-running app.
    ///
    /// After handling, emits a `terminated` event and returns a success response.
    /// The session layer transitions to `Disconnecting` after this call.
    pub(super) async fn handle_disconnect(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: disconnect");

        let args: crate::protocol::types::DisconnectArguments = request
            .arguments
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        if args.terminate_debuggee.unwrap_or(false) {
            // IDE wants the app stopped — terminate the Flutter process.
            tracing::debug!("disconnect: terminateDebuggee=true — stopping app");
            if let Err(e) = self.backend.stop_app().await {
                tracing::warn!("stop_app failed during disconnect: {}", e);
                // Non-fatal: continue the disconnect sequence even if stop_app fails.
            }
        } else {
            // Default: resume any paused isolates so the app keeps running.
            let paused = std::mem::take(&mut self.paused_isolates);
            for isolate_id in &paused {
                tracing::debug!(
                    "disconnect: resuming paused isolate {} (terminateDebuggee=false)",
                    isolate_id
                );
                if let Err(e) = self.backend.resume(isolate_id, None, None).await {
                    tracing::warn!("resume({}) failed during disconnect: {}", isolate_id, e);
                }
            }
        }

        // Note: the `terminated` event is emitted by the session layer, not here,
        // so that the synchronous `handle_request` return value includes the event
        // in the correct position (before the response, per DAP spec). When the
        // adapter is used standalone (e.g., in unit tests), the caller is responsible
        // for emitting the terminated event if needed.
        DapResponse::success(request, None)
    }

    /// Handle the `evaluate` request.
    ///
    /// Evaluates an expression in the context of the current debug session.
    /// Dispatches to [`evaluate::handle_evaluate`] which calls either
    /// `evaluateInFrame` (when a `frameId` is provided) or `evaluate` on the
    /// root library (when no `frameId` is given).
    ///
    /// # Magic expressions
    ///
    /// `$_threadException` — Returns the current exception when the isolate is
    /// paused at an exception. The returned value includes a `variablesReference`
    /// so that the IDE can expand the exception's fields.
    ///
    /// # Error Handling
    ///
    /// - No paused isolate → DAP error response
    /// - Invalid frame ID → DAP error response
    /// - VM Service error → DAP error response with the error message
    pub(super) async fn handle_evaluate(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: evaluate");

        // Intercept the `$_threadException` magic expression before delegating
        // to the standard evaluation path. The adapter resolves it directly from
        // the stored exception ref so no VM round-trip is needed.
        if let Some(args) = request.arguments.as_ref() {
            let expression = args
                .get("expression")
                .and_then(|e| e.as_str())
                .unwrap_or("");
            if expression == "$_threadException" {
                return self.handle_evaluate_thread_exception(request);
            }
        }

        let paused = self.most_recent_paused_isolate().map(|s| s.to_string());
        crate::adapter::evaluate::handle_evaluate(
            &self.backend,
            &self.frame_store,
            &mut self.var_store,
            paused.as_deref(),
            request,
        )
        .await
    }

    /// Handle the `$_threadException` magic evaluate expression.
    ///
    /// Returns the current exception `InstanceRef` for the most recently
    /// paused isolate. The result includes a non-zero `variablesReference`
    /// so the IDE can expand the exception's fields.
    ///
    /// Returns an error response if no exception is currently stored
    /// (i.e., the isolate is not paused at an exception).
    fn handle_evaluate_thread_exception(&mut self, request: &DapRequest) -> DapResponse {
        // Find the most recently paused isolate's thread ID.
        let thread_id = self
            .most_recent_paused_isolate()
            .and_then(|iso| self.thread_map.thread_id_for(iso));

        let thread_id = match thread_id {
            Some(tid) => tid,
            None => {
                return DapResponse::error(
                    request,
                    "$_threadException: no paused isolate available",
                )
            }
        };

        let exc = match self.exception_refs.get(&thread_id) {
            Some(e) => e,
            None => {
                return DapResponse::error(request, "$_threadException: not paused at an exception")
            }
        };

        // Format the exception value using the instance_ref_to_variable helper.
        let class_name = exc
            .instance_ref
            .get("classRef")
            .or_else(|| exc.instance_ref.get("class"))
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("Exception")
            .to_string();
        let instance_ref = exc.instance_ref.clone();
        let isolate_id = exc.isolate_id.clone();

        // Convert to a DapVariable to get both display value and variablesReference.
        let var = self.instance_ref_to_variable(&class_name, &instance_ref, &isolate_id);

        let body = serde_json::json!({
            "result": var.value,
            "type": var.type_field,
            "variablesReference": var.variables_reference,
        });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `source` DAP request.
    ///
    /// Returns the source text for a source reference ID that was previously
    /// allocated during `stackTrace` responses. The reference maps to a Dart VM
    /// `Script` object; the source text is fetched via `getObject`.
    ///
    /// # Errors
    ///
    /// - Unknown or cleared `sourceReference` → DAP error response
    /// - Backend `getObject` failure → DAP error response with the error message
    pub(super) async fn handle_source(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: source");

        // Parse source reference from the request arguments.
        let source_ref = match request.arguments.as_ref() {
            Some(args) => {
                // The DAP `source` request arguments may have either `sourceReference`
                // at the top level or nested inside a `source` object.
                let top_level = args.get("sourceReference").and_then(|v| v.as_i64());
                let nested = args
                    .get("source")
                    .and_then(|s| s.get("sourceReference"))
                    .and_then(|v| v.as_i64());
                match top_level.or(nested) {
                    Some(r) => r,
                    None => {
                        return DapResponse::error(
                            request,
                            "'source' request requires 'sourceReference'".to_string(),
                        )
                    }
                }
            }
            None => {
                return DapResponse::error(
                    request,
                    "'source' request requires arguments".to_string(),
                )
            }
        };

        // Look up the script information for this reference.
        let entry = match self.source_reference_store.get(source_ref) {
            Some(e) => e,
            None => {
                return DapResponse::error(
                    request,
                    format!("Unknown source reference: {source_ref}"),
                )
            }
        };

        // Fetch the source text from the VM Service.
        match self
            .backend
            .get_source(&entry.isolate_id, &entry.script_id)
            .await
        {
            Ok(source_text) => {
                let body = serde_json::json!({
                    "content": source_text,
                    "mimeType": "text/x-dart",
                });
                DapResponse::success(request, Some(body))
            }
            Err(e) => DapResponse::error(
                request,
                format!("Failed to fetch source for reference {source_ref}: {e}"),
            ),
        }
    }

    /// Handle the `loadedSources` DAP request.
    ///
    /// Returns all Dart scripts currently loaded in the most recently active
    /// isolate as DAP `Source` objects. This enables the "Loaded Scripts"
    /// explorer panel in VS Code and other IDEs.
    ///
    /// # Source categorization
    ///
    /// | URI prefix              | Treatment                                             |
    /// |-------------------------|-------------------------------------------------------|
    /// | `file://`               | Resolved to a local filesystem path                   |
    /// | `package:`              | Local path if resolvable; `sourceReference` otherwise |
    /// | `dart:`                 | `sourceReference > 0`; `presentationHint: "deemphasize"` |
    /// | `org-dartlang-sdk:`     | `sourceReference > 0`; `presentationHint: "deemphasize"` |
    /// | `eval:` or `dart:_*`   | Filtered out (generated / internal)                   |
    ///
    /// # Isolate selection
    ///
    /// Uses the most recently paused isolate if one is available, otherwise
    /// falls back to the primary (first registered) isolate. Returns an error
    /// if no isolate is known.
    pub(super) async fn handle_loaded_sources(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: loadedSources");

        // Prefer a paused isolate for script enumeration; fall back to primary.
        let isolate_id = match self
            .most_recent_paused_isolate()
            .map(|s| s.to_string())
            .or_else(|| self.primary_isolate_id())
        {
            Some(id) => id,
            None => {
                return DapResponse::error(request, "loadedSources: no active isolate available")
            }
        };

        let scripts_response = match self.backend.get_scripts(&isolate_id).await {
            Ok(v) => v,
            Err(e) => {
                return DapResponse::error(
                    request,
                    format!("loadedSources: get_scripts failed: {e}"),
                )
            }
        };

        let empty_vec = Vec::new();
        let scripts = scripts_response
            .get("scripts")
            .and_then(|s| s.as_array())
            .unwrap_or(&empty_vec);

        let sources: Vec<DapSource> = scripts
            .iter()
            .filter_map(|script| {
                let uri = script.get("uri")?.as_str()?;
                let script_id = script.get("id")?.as_str()?;

                // Filter out generated `eval:` sources and Dart internal libraries.
                if uri.starts_with("eval:") || uri.contains("dart:_") {
                    return None;
                }

                Some(build_source_from_uri(
                    uri,
                    script_id,
                    &mut self.source_reference_store,
                    &isolate_id,
                    None, // project_root not available in DapAdapter
                ))
            })
            .collect();

        tracing::debug!("loadedSources: returning {} sources", sources.len());
        DapResponse::success(request, Some(serde_json::json!({ "sources": sources })))
    }

    /// Handle the `hotReload` custom DAP request.
    ///
    /// Triggers a Flutter hot reload through the backend's TEA message bus.
    /// The `arguments.reason` field is optional and informational — it does
    /// not change reload behavior.
    ///
    /// When the client advertises `supportsProgressReporting`, emits:
    /// - `progressStart` (title: `"Hot Reload"`, `cancellable: false`)
    /// - `progressEnd` on completion (even on failure, per DAP spec)
    ///
    /// Always emits `dart.hotReloadComplete` on success, as expected by the
    /// Dart-Code VS Code extension for updating its internal state.
    ///
    /// Compatible with the VS Code Dart extension's `hotReload` custom request.
    pub(super) async fn handle_hot_reload(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: hotReload");

        let progress_id = if self.client_supports_progress {
            let id = self.alloc_progress_id();
            self.send_event(
                "progressStart",
                Some(serde_json::json!({
                    "progressId": id,
                    "title": "Hot Reload",
                    "cancellable": false,
                })),
            )
            .await;
            Some(id)
        } else {
            None
        };

        let result = self.backend.hot_reload().await;

        // Always close the progress indicator, even on failure, so the IDE
        // does not display a stale spinner indefinitely.
        if let Some(ref id) = progress_id {
            self.send_event("progressEnd", Some(serde_json::json!({ "progressId": id })))
                .await;
        }

        match result {
            Ok(()) => {
                tracing::debug!("Hot reload dispatched successfully");
                // dart.hotReloadComplete is a custom event expected by the
                // Dart-Code extension to update its internal session state.
                self.send_event("dart.hotReloadComplete", Some(serde_json::json!({})))
                    .await;
                DapResponse::success(request, None)
            }
            Err(e) => {
                tracing::warn!("Hot reload failed: {}", e);
                DapResponse::error(request, format!("Hot reload failed: {e}"))
            }
        }
    }

    /// Handle the `hotRestart` custom DAP request.
    ///
    /// Triggers a Flutter hot restart through the backend's TEA message bus.
    /// Hot restart creates a new Dart isolate, so all variable references
    /// and frame IDs are invalidated after restart.
    ///
    /// The `arguments.reason` field is optional and informational — it does
    /// not change restart behavior.
    ///
    /// When the client advertises `supportsProgressReporting`, emits:
    /// - `progressStart` (title: `"Hot Restart"`, `cancellable: false`)
    /// - `progressEnd` on completion (even on failure, per DAP spec)
    ///
    /// Always emits `dart.hotRestartComplete` on success, as expected by the
    /// Dart-Code VS Code extension for updating its internal state.
    ///
    /// Compatible with the VS Code Dart extension's `hotRestart` custom request.
    pub(super) async fn handle_hot_restart(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: hotRestart");

        let progress_id = if self.client_supports_progress {
            let id = self.alloc_progress_id();
            self.send_event(
                "progressStart",
                Some(serde_json::json!({
                    "progressId": id,
                    "title": "Hot Restart",
                    "cancellable": false,
                })),
            )
            .await;
            Some(id)
        } else {
            None
        };

        let result = self.backend.hot_restart().await;

        // Always close the progress indicator, even on failure, so the IDE
        // does not display a stale spinner indefinitely.
        if let Some(ref id) = progress_id {
            self.send_event("progressEnd", Some(serde_json::json!({ "progressId": id })))
                .await;
        }

        match result {
            Ok(()) => {
                tracing::debug!("Hot restart dispatched successfully");
                // dart.hotRestartComplete is a custom event expected by the
                // Dart-Code extension to update its internal session state.
                self.send_event("dart.hotRestartComplete", Some(serde_json::json!({})))
                    .await;
                DapResponse::success(request, None)
            }
            Err(e) => {
                tracing::warn!("Hot restart failed: {}", e);
                DapResponse::error(request, format!("Hot restart failed: {e}"))
            }
        }
    }

    /// Handle the `restartFrame` DAP request.
    ///
    /// Rewinds execution to the start of a selected stack frame using the Dart
    /// VM Service's `Rewind` step mode. This enables the "Restart Frame" action
    /// in IDE debuggers, allowing developers to re-execute a function without
    /// restarting the entire application.
    ///
    /// # Async boundary guard
    ///
    /// Frames at or above the first `AsyncSuspensionMarker` cannot be rewound.
    /// The Dart VM only supports rewinding synchronous frames below the first
    /// async suspension boundary. Attempting to rewind past it would cause the
    /// VM to return an error. This handler rejects such requests with a clear
    /// error message before making any backend call.
    ///
    /// # Post-rewind behaviour
    ///
    /// After a successful rewind, the VM pauses at the rewound frame and emits
    /// a `PauseInterrupted` or `PauseBreakpoint` event. The existing
    /// [`handle_debug_event`] handler translates this to a DAP `stopped` event
    /// automatically — no special handling is needed here.
    pub(super) async fn handle_restart_frame(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: restartFrame");

        let args = match parse_args::<RestartFrameArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        // Look up the frame in the per-stop frame store.
        let frame_ref = match self.frame_store.lookup(args.frame_id) {
            Some(fr) => fr.clone(),
            None => {
                return DapResponse::error(
                    request,
                    format!(
                        "Invalid or stale frame ID {} — did the program resume?",
                        args.frame_id
                    ),
                )
            }
        };

        // Guard: reject frames at or above the first async suspension boundary.
        // The VM cannot rewind through async suspension markers.
        if let Some(first_async_index) = self.first_async_marker_index {
            if frame_ref.frame_index >= first_async_index {
                tracing::debug!(
                    "restartFrame: frame {} is at or above async marker at index {} — rejecting",
                    frame_ref.frame_index,
                    first_async_index,
                );
                return DapResponse::error(
                    request,
                    "Cannot restart frame: target frame is at or above an async suspension boundary",
                );
            }
        }

        // Invalidate stopped-state references before rewinding.
        self.on_resume();

        match self
            .backend
            .resume(
                &frame_ref.isolate_id,
                Some(StepMode::Rewind),
                Some(frame_ref.frame_index),
            )
            .await
        {
            Ok(()) => {
                tracing::debug!(
                    "restartFrame: rewound to frame {} in isolate {}",
                    frame_ref.frame_index,
                    frame_ref.isolate_id,
                );
                DapResponse::success(request, Some(serde_json::json!({})))
            }
            Err(e) => DapResponse::error(request, format!("restartFrame failed: {e}")),
        }
    }

    /// Handle the `callService` custom DAP request.
    ///
    /// Forwards an arbitrary VM Service RPC call to the backend's
    /// `call_service` method. This is a Dart-specific custom request used by
    /// the VS Code Dart extension to invoke service extensions such as:
    ///
    /// | Method | Purpose |
    /// |---|---|
    /// | `ext.flutter.debugDumpApp` | Widget inspector dump |
    /// | `ext.flutter.showPerformanceOverlay` | Toggle perf overlay |
    /// | `ext.flutter.debugPaint` | Toggle debug painting |
    /// | `ext.flutter.reassemble` | Trigger hot reload |
    ///
    /// ## Arguments
    ///
    /// The request body must include a `"method"` string field identifying the
    /// VM Service RPC. An optional `"params"` object is forwarded verbatim.
    ///
    /// ## Security
    ///
    /// All invocations are logged at `debug` level for auditability. No
    /// method-level filtering is applied — the VM Service itself handles
    /// authorization. The DAP server is already bound to `127.0.0.1` by
    /// default so only local callers can reach this endpoint.
    pub(super) async fn handle_call_service(&mut self, request: &DapRequest) -> DapResponse {
        let args = match request.arguments.as_ref() {
            Some(a) => a,
            None => return DapResponse::error(request, "callService: missing arguments"),
        };

        let method = match args.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => return DapResponse::error(request, "callService: missing 'method' argument"),
        };

        let params = args.get("params").cloned();

        tracing::debug!("callService: method={}, params={:?}", method, params);

        match self.backend.call_service(method, params).await {
            Ok(result) => {
                DapResponse::success(request, Some(serde_json::json!({ "result": result })))
            }
            Err(e) => DapResponse::error(request, format!("callService failed: {}", e)),
        }
    }

    /// Handle the `exceptionInfo` DAP request.
    ///
    /// Returns structured exception data when the debugger is paused at an
    /// exception. Provides richer detail than the basic `stopped` event —
    /// the IDE displays this in the exception details dialog.
    ///
    /// # Response fields
    ///
    /// | Field | Content |
    /// |---|---|
    /// | `exceptionId` | The VM object ID of the exception (e.g., `"objects/12345"`) |
    /// | `description` | Result of `toString()` on the exception |
    /// | `breakMode` | `"always"`, `"unhandled"`, or `"never"` based on the current pause mode |
    /// | `details.typeName` | The Dart class name of the exception (e.g., `"FormatException"`) |
    /// | `details.message` | Same as `description` (for IDE detail panels) |
    /// | `details.stackTrace` | Result of `stackTrace?.toString()` if available |
    /// | `details.evaluateName` | `"$_threadException"` for IDE expression evaluation |
    ///
    /// # Errors
    ///
    /// - No exception available for the given thread → DAP error response
    /// - `toString()` evaluation failure → `description` falls back to the class name
    /// - `stackTrace?.toString()` failure → `details.stackTrace` is absent
    pub(super) async fn handle_exception_info(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: exceptionInfo");

        let args = match parse_args::<ExceptionInfoArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        // Look up the stored exception reference for this thread.
        let exc = match self.exception_refs.get(&args.thread_id) {
            Some(e) => e.clone(),
            None => {
                return DapResponse::error(
                    request,
                    format!(
                        "No exception available for thread {} — not paused at an exception",
                        args.thread_id
                    ),
                )
            }
        };

        let isolate_id = exc.isolate_id.clone();

        // Extract the VM object ID from the InstanceRef.
        let exception_id = exc
            .instance_ref
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Extract the exception class name for `typeName`.
        let type_name = exc
            .instance_ref
            .get("classRef")
            .or_else(|| exc.instance_ref.get("class"))
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("Exception")
            .to_string();

        // Call toString() on the exception for the description.
        let description = if !exception_id.is_empty() {
            match self
                .backend
                .evaluate(&isolate_id, &exception_id, "toString()")
                .await
            {
                Ok(result) => result
                    .get("valueAsString")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&type_name)
                    .to_string(),
                Err(_) => type_name.clone(),
            }
        } else {
            type_name.clone()
        };

        // Try to get the stack trace string via stackTrace?.toString().
        let stack_trace_str = if !exception_id.is_empty() {
            match self
                .backend
                .evaluate(&isolate_id, &exception_id, "stackTrace?.toString()")
                .await
            {
                Ok(result) => result
                    .get("valueAsString")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                Err(_) => None,
            }
        } else {
            None
        };

        // Map the current exception pause mode to the DAP breakMode string.
        let break_mode = match self.exception_mode {
            crate::adapter::types::DapExceptionPauseMode::All => "always",
            crate::adapter::types::DapExceptionPauseMode::Unhandled => "unhandled",
            crate::adapter::types::DapExceptionPauseMode::None => "never",
        };

        // Build the optional details object.
        let mut details = serde_json::json!({
            "typeName": type_name,
            "message": description,
            "evaluateName": "$_threadException",
        });
        if let Some(st) = stack_trace_str {
            details["stackTrace"] = serde_json::Value::String(st);
        }

        let body = serde_json::json!({
            "exceptionId": exception_id,
            "description": description,
            "breakMode": break_mode,
            "details": details,
        });

        tracing::debug!(
            "exceptionInfo: thread={} type={} breakMode={}",
            args.thread_id,
            type_name,
            break_mode,
        );

        DapResponse::success(request, Some(body))
    }

    /// Handle the `updateDebugOptions` custom DAP request.
    ///
    /// Toggles whether the debugger steps into Dart SDK libraries (`dart:`
    /// URIs) and/or external package libraries. Changes are applied immediately
    /// to all currently-known isolates via `setLibraryDebuggable` VM Service
    /// RPC calls.
    ///
    /// # Arguments (in `request.arguments`)
    ///
    /// | Field | Type | Effect |
    /// |---|---|---|
    /// | `debugSdkLibraries` | `bool` | Allow stepping into `dart:` libraries |
    /// | `debugExternalPackageLibraries` | `bool` | Allow stepping into external packages |
    ///
    /// Either field may be absent; absent fields leave the existing setting
    /// unchanged.
    ///
    /// # App code
    ///
    /// Libraries whose URI matches `package:<app_package_name>/` are always
    /// debuggable regardless of these settings.
    pub(super) async fn handle_update_debug_options(
        &mut self,
        request: &DapRequest,
    ) -> DapResponse {
        tracing::debug!("DAP adapter: updateDebugOptions");

        let args = match request.arguments.as_ref() {
            Some(a) => a,
            None => return DapResponse::error(request, "updateDebugOptions: missing arguments"),
        };

        // Update settings from the incoming arguments. Fields that are absent
        // leave the existing setting unchanged.
        if let Some(debug_sdk) = args.get("debugSdkLibraries").and_then(|v| v.as_bool()) {
            self.debug_sdk_libraries = debug_sdk;
        }
        if let Some(debug_external) = args
            .get("debugExternalPackageLibraries")
            .and_then(|v| v.as_bool())
        {
            self.debug_external_package_libraries = debug_external;
        }

        // Collect all current isolate IDs before the async loop so we don't
        // hold a borrow on `self` while calling async backend methods.
        let isolate_ids: Vec<String> = self
            .thread_map
            .all_threads()
            .map(|(_, iso)| iso.to_string())
            .collect();

        // Apply the current library-debuggability settings to every known isolate.
        for isolate_id in &isolate_ids {
            if let Err(e) = self.apply_library_debuggability(isolate_id).await {
                tracing::warn!(
                    "updateDebugOptions: failed to apply library debuggability to {}: {}",
                    isolate_id,
                    e,
                );
            }
        }

        tracing::debug!(
            "updateDebugOptions applied to {} isolate(s): sdk={} external={}",
            isolate_ids.len(),
            self.debug_sdk_libraries,
            self.debug_external_package_libraries,
        );

        DapResponse::success(request, Some(serde_json::json!({})))
    }

    /// Apply library debuggability settings to all libraries in an isolate.
    ///
    /// Fetches the isolate's library list via `getIsolate` and calls
    /// `setLibraryDebuggable` for each library according to the current
    /// `debug_sdk_libraries` and `debug_external_package_libraries` flags.
    ///
    /// # Classification
    ///
    /// | URI prefix | Debuggable? |
    /// |---|---|
    /// | `dart:` | `self.debug_sdk_libraries` |
    /// | `package:<app>/` | Always `true` (app code) |
    /// | `package:<other>/` | `self.debug_external_package_libraries` |
    /// | `file://` | Always `true` (app code) |
    /// | Other | Always `true` |
    ///
    /// Failures from individual `setLibraryDebuggable` calls are logged as
    /// warnings and do not abort processing of remaining libraries.
    pub(super) async fn apply_library_debuggability(&self, isolate_id: &str) -> Result<(), String> {
        let isolate = self
            .backend
            .get_isolate(isolate_id)
            .await
            .map_err(|e| format!("get_isolate failed: {e}"))?;

        let empty_vec = Vec::new();
        let libraries = isolate
            .get("libraries")
            .and_then(|l| l.as_array())
            .unwrap_or(&empty_vec);

        for lib in libraries {
            let lib_id = lib.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let uri = lib.get("uri").and_then(|u| u.as_str()).unwrap_or("");

            if lib_id.is_empty() {
                continue;
            }

            let is_debuggable = if uri.starts_with("dart:") {
                self.debug_sdk_libraries
            } else if uri.starts_with("package:") && !self.is_app_package(uri) {
                self.debug_external_package_libraries
            } else {
                // App code (file://, package:<app>/, or anything else) is
                // always debuggable.
                true
            };

            self.backend
                .set_library_debuggable(isolate_id, lib_id, is_debuggable)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        "Failed to set library debuggability for {} ({}): {}",
                        uri,
                        lib_id,
                        e
                    );
                });
        }

        Ok(())
    }

    /// Handle the `breakpointLocations` DAP request.
    ///
    /// Returns valid breakpoint positions for a given source file and line range.
    /// This enables IDEs to show valid breakpoint markers when the user hovers
    /// over the editor gutter, and supports column breakpoints (multiple
    /// breakpoints on a single line).
    ///
    /// ## Implementation
    ///
    /// 1. Resolves the source file path to a Dart `file://` URI.
    /// 2. Finds the matching script in the isolate's script list.
    /// 3. Calls `getSourceReport` with `PossibleBreakpoints` to get valid positions.
    /// 4. Filters the positions to the requested line range.
    ///
    /// Token position → line/column mapping uses the script's `tokenPosTable`
    /// when available. Each row is `[line, tokenPos, col, tokenPos, col, ...]`.
    /// If the table is absent, positions are returned at line level only.
    ///
    /// ## Errors
    ///
    /// - No active isolate → error response
    /// - Missing source path → error response
    /// - Script not found for URI → returns empty breakpoints array (file may
    ///   not be loaded yet — not a fatal error)
    /// - `getSourceReport` failure → error response
    pub(super) async fn handle_breakpoint_locations(
        &mut self,
        request: &DapRequest,
    ) -> DapResponse {
        tracing::debug!("DAP adapter: breakpointLocations");

        let args = match parse_args::<BreakpointLocationsArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let source_path = match args.source.path.as_deref() {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => {
                return DapResponse::error(request, "breakpointLocations: source path is required")
            }
        };

        // Pick the best available isolate (most recently paused, or primary).
        let isolate_id = match self
            .most_recent_paused_isolate()
            .map(|s| s.to_string())
            .or_else(|| self.primary_isolate_id())
        {
            Some(id) => id,
            None => {
                return DapResponse::error(
                    request,
                    "breakpointLocations: no active isolate available",
                )
            }
        };

        // Convert the filesystem path to a Dart VM URI.
        let uri = path_to_dart_uri(&source_path);

        // Retrieve the script list to find the script ID for this URI.
        let scripts_response = match self.backend.get_scripts(&isolate_id).await {
            Ok(v) => v,
            Err(e) => {
                return DapResponse::error(
                    request,
                    format!("breakpointLocations: get_scripts failed: {e}"),
                )
            }
        };

        // Find the script whose URI matches the requested file URI.
        let script_id = find_script_id_by_uri(&scripts_response, &uri);
        let script_id = match script_id {
            Some(id) => id,
            None => {
                // Script not found — the file may not be loaded yet or may be
                // outside the Dart source tree. Return an empty set rather than
                // an error so the IDE does not treat this as a hard failure.
                tracing::debug!(
                    "breakpointLocations: script not found for URI '{}' — returning empty list",
                    uri
                );
                let empty: Vec<BreakpointLocation> = Vec::new();
                let body = serde_json::json!({ "breakpoints": empty });
                return DapResponse::success(request, Some(body));
            }
        };

        // Call getSourceReport to get possible breakpoint positions for the script.
        let report = match self
            .backend
            .get_source_report(
                &isolate_id,
                &script_id,
                &["PossibleBreakpoints"],
                None,
                None,
            )
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return DapResponse::error(
                    request,
                    format!("breakpointLocations: getSourceReport failed: {e}"),
                )
            }
        };

        // Extract token-to-line/column mapping from the script object if available.
        // The script's tokenPosTable is a 2D array where each row is:
        //   [line, tokenPos, col, tokenPos, col, ...]
        // We build a HashMap from tokenPos → (line, col) for fast lookup.
        let token_pos_map = build_token_pos_map(&report);

        // Extract breakpoint locations filtered to the requested line range.
        let end_line = args.end_line.unwrap_or(args.line);
        let locations = extract_breakpoint_locations(&report, &token_pos_map, args.line, end_line);

        tracing::debug!(
            "breakpointLocations: {} location(s) for {} lines {}-{}",
            locations.len(),
            source_path,
            args.line,
            end_line,
        );

        let body = serde_json::json!({ "breakpoints": locations });
        DapResponse::success(request, Some(body))
    }

    /// Return `true` if the given `package:` URI belongs to the app's own package.
    ///
    /// A URI is an app package URI when it starts with
    /// `package:<app_package_name>/`. If `app_package_name` is empty, no
    /// `package:` URI is considered an app URI (they are all treated as
    /// external).
    ///
    /// # Examples
    ///
    /// ```text
    /// app_package_name = "my_app"
    /// "package:my_app/main.dart"         → true
    /// "package:my_app/src/widget.dart"   → true
    /// "package:flutter/material.dart"    → false
    /// "dart:core"                        → false (not a package: URI)
    /// ```
    pub(super) fn is_app_package(&self, uri: &str) -> bool {
        if self.app_package_name.is_empty() {
            return false;
        }
        // Match "package:<name>/" exactly so that a package named "my_app"
        // does not accidentally match "my_app_test".
        let prefix = format!("package:{}/", self.app_package_name);
        uri.starts_with(&prefix)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Free helper functions
// ─────────────────────────────────────────────────────────────────────────────

/// Parse the `arguments` field of a [`DapRequest`] as `T`.
///
/// Returns `Err` with a human-readable message if the field is absent or
/// cannot be deserialized.
pub(crate) fn parse_args<T: serde::de::DeserializeOwned>(
    request: &DapRequest,
) -> Result<T, String> {
    match &request.arguments {
        Some(v) => {
            serde_json::from_value(v.clone()).map_err(|e| format!("invalid arguments: {}", e))
        }
        None => Err(format!("'{}' request requires arguments", request.command)),
    }
}

/// Convert an absolute filesystem path to a `file://` URI suitable for the
/// Dart VM Service.
///
/// # Phase 3 Note
///
/// This returns a plain `file://` URI. Full `package:` URI resolution (which
/// requires reading `.dart_tool/package_config.json`) is deferred to Phase 4.
/// The Dart VM Service accepts both `file://` and `package:` URIs for
/// `addBreakpointWithScriptUri`.
pub(crate) fn path_to_dart_uri(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    // Pass through paths that already have a URI scheme (file://, package:, etc.).
    if path.starts_with("file://") || path.starts_with("package:") || path.starts_with("dart:") {
        return path.to_string();
    }
    format!("file://{}", path)
}

/// Build a [`DapBreakpoint`] from a tracked [`BreakpointEntry`].
///
/// The `source` from the original `setBreakpoints` request is echoed back so
/// the IDE can correlate the response breakpoint with the source file.
pub(crate) fn entry_to_dap_breakpoint(
    entry: &breakpoints::BreakpointEntry,
    source: &DapSource,
) -> DapBreakpoint {
    DapBreakpoint {
        id: Some(entry.dap_id),
        verified: entry.verified,
        message: if entry.verified {
            None
        } else {
            Some("Breakpoint not yet resolved".to_string())
        },
        source: Some(source.clone()),
        line: entry.line.map(|l| l as i64),
        column: entry.column.map(|c| c as i64),
        ..Default::default()
    }
}

/// Map a set of DAP exception filter IDs to a [`DapExceptionPauseMode`].
///
/// `"All"` takes precedence when both `"All"` and `"Unhandled"` are present.
/// Unknown filter strings are not handled here — callers should validate first.
pub(crate) fn exception_filter_to_mode(filters: &[String]) -> DapExceptionPauseMode {
    if filters.iter().any(|f| f == "All") {
        DapExceptionPauseMode::All
    } else if filters.iter().any(|f| f == "Unhandled") {
        DapExceptionPauseMode::Unhandled
    } else {
        DapExceptionPauseMode::None
    }
}

/// Find the script ID whose URI matches the given Dart URI in a `ScriptList` response.
///
/// The `scripts_response` is the JSON value returned by `get_scripts()`, which
/// wraps an array under the `"scripts"` key. Each element has `"id"` and `"uri"` fields.
///
/// Returns `None` when no script matches.
pub(crate) fn find_script_id_by_uri(
    scripts_response: &serde_json::Value,
    uri: &str,
) -> Option<String> {
    let scripts = scripts_response.get("scripts")?.as_array()?;
    for script in scripts {
        let script_uri = script.get("uri")?.as_str()?;
        if script_uri == uri {
            let id = script.get("id")?.as_str()?;
            return Some(id.to_string());
        }
    }
    None
}

/// Build a map from VM token position → `(line, column)` from a `getSourceReport` response.
///
/// The Dart VM embeds the `tokenPosTable` inside the `scripts` array of the
/// source report. Each script entry may have a `tokenPosTable` field which is a
/// 2-dimensional array. Each row has the form:
///
/// ```text
/// [line, tokenPos, column, tokenPos, column, ...]
/// ```
///
/// where `line` is the 1-based source line number and each subsequent pair is a
/// token position followed by its 1-based column offset.
///
/// Returns a `HashMap<i64, (i64, i64)>` mapping `tokenPos → (line, column)`.
/// Returns an empty map when no `tokenPosTable` is present.
pub(crate) fn build_token_pos_map(
    report: &serde_json::Value,
) -> std::collections::HashMap<i64, (i64, i64)> {
    let mut map = std::collections::HashMap::new();

    let scripts = match report.get("scripts").and_then(|s| s.as_array()) {
        Some(s) => s,
        None => return map,
    };

    for script in scripts {
        let table = match script.get("tokenPosTable").and_then(|t| t.as_array()) {
            Some(t) => t,
            None => continue,
        };

        for row in table {
            let row = match row.as_array() {
                Some(r) => r,
                None => continue,
            };
            // row[0] is the line number; subsequent pairs are (tokenPos, column).
            if row.len() < 3 {
                continue;
            }
            let line = match row[0].as_i64() {
                Some(l) => l,
                None => continue,
            };
            // Iterate over (tokenPos, column) pairs starting at index 1.
            let mut i = 1;
            while i + 1 < row.len() {
                if let (Some(token_pos), Some(col)) = (row[i].as_i64(), row[i + 1].as_i64()) {
                    map.insert(token_pos, (line, col));
                }
                i += 2;
            }
        }
    }

    map
}

/// Extract [`BreakpointLocation`] objects from a `getSourceReport` response.
///
/// Iterates the `ranges` array and collects all `possibleBreakpoints` token
/// positions. Each token position is looked up in `token_pos_map` to obtain
/// a `(line, column)` pair. Only positions whose line falls within
/// `[start_line, end_line]` (both inclusive, 1-based) are included.
///
/// When a token position is not present in `token_pos_map`, the position is
/// skipped (column-level accuracy requires the `tokenPosTable`).
pub(crate) fn extract_breakpoint_locations(
    report: &serde_json::Value,
    token_pos_map: &std::collections::HashMap<i64, (i64, i64)>,
    start_line: i64,
    end_line: i64,
) -> Vec<BreakpointLocation> {
    let mut locations: Vec<BreakpointLocation> = Vec::new();

    let ranges = match report.get("ranges").and_then(|r| r.as_array()) {
        Some(r) => r,
        None => return locations,
    };

    for range in ranges {
        let possible = match range.get("possibleBreakpoints").and_then(|p| p.as_array()) {
            Some(p) => p,
            None => continue,
        };

        for token_pos_val in possible {
            let token_pos = match token_pos_val.as_i64() {
                Some(t) => t,
                None => continue,
            };

            if let Some(&(line, col)) = token_pos_map.get(&token_pos) {
                if line >= start_line && line <= end_line {
                    locations.push(BreakpointLocation {
                        line,
                        column: Some(col),
                        end_line: None,
                        end_column: None,
                    });
                }
            }
        }
    }

    // Sort by line then column for deterministic output.
    locations.sort_by(|a, b| a.line.cmp(&b.line).then(a.column.cmp(&b.column)));
    // Deduplicate identical positions.
    locations.dedup_by(|a, b| a.line == b.line && a.column == b.column);

    locations
}
