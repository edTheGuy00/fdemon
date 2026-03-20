//! # Request Handlers
//!
//! DapAdapter methods for dispatching and handling DAP protocol requests.

use crate::adapter::backend::DebugBackend;
use crate::adapter::breakpoints;
use crate::adapter::types::{DapExceptionPauseMode, StepMode};
use crate::adapter::DapAdapter;
use crate::protocol::types::{
    AttachRequestArguments, ContinueArguments, DapBreakpoint, DapSource, DapThread, PauseArguments,
    SetBreakpointsArguments, SetExceptionBreakpointsArguments, StepArguments,
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
            "hotReload" => self.handle_hot_reload(request).await,
            "hotRestart" => self.handle_hot_restart(request).await,
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

        match self.backend.resume(&isolate_id, None).await {
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

        match self.backend.resume(&isolate_id, Some(mode)).await {
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
                if let Err(e) = self.backend.resume(isolate_id, None).await {
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

    /// Handle the `hotReload` custom DAP request.
    ///
    /// Triggers a Flutter hot reload through the backend's TEA message bus.
    /// The `arguments.reason` field is optional and informational — it does
    /// not change reload behavior.
    ///
    /// Compatible with the VS Code Dart extension's `hotReload` custom request.
    pub(super) async fn handle_hot_reload(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: hotReload");
        match self.backend.hot_reload().await {
            Ok(()) => {
                tracing::debug!("Hot reload dispatched successfully");
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
    /// Compatible with the VS Code Dart extension's `hotRestart` custom request.
    pub(super) async fn handle_hot_restart(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: hotRestart");
        match self.backend.hot_restart().await {
            Ok(()) => {
                tracing::debug!("Hot restart dispatched successfully");
                DapResponse::success(request, None)
            }
            Err(e) => {
                tracing::warn!("Hot restart failed: {}", e);
                DapResponse::error(request, format!("Hot restart failed: {e}"))
            }
        }
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
