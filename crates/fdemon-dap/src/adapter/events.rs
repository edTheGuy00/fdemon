//! # Debug Event Handling
//!
//! DapAdapter methods for handling VM Service debug events and emitting the
//! corresponding DAP protocol events to the IDE client.

use crate::adapter::backend::DebugBackend;
use crate::adapter::breakpoints;
use crate::adapter::stack::dart_uri_to_path;
use crate::adapter::types::{
    log_level_to_category, DapExceptionPauseMode, DebugEvent, PauseReason,
};
use crate::adapter::DapAdapter;
use crate::protocol::types::DapSource;
use crate::{DapEvent, DapMessage};

impl<B: DebugBackend> DapAdapter<B> {
    /// Notify the adapter of a VM Service debug event.
    ///
    /// Called by the Engine integration layer when a debug stream event arrives.
    /// The adapter translates it to the appropriate DAP events and sends them
    /// via [`event_tx`](DapAdapter::event_tx).
    pub async fn handle_debug_event(&mut self, event: DebugEvent) {
        match event {
            DebugEvent::IsolateStart { isolate_id, name } => {
                let thread_id = self.thread_map.get_or_create(&isolate_id);
                self.thread_names.insert(thread_id, name.clone());
                tracing::debug!(
                    "Isolate started: {} (thread {}), name: {}",
                    isolate_id,
                    thread_id,
                    name
                );
                let body = serde_json::json!({
                    "reason": "started",
                    "threadId": thread_id,
                });
                self.send_event("thread", Some(body)).await;
            }

            DebugEvent::IsolateExit { isolate_id } => {
                if let Some(thread_id) = self.thread_map.remove(&isolate_id) {
                    self.thread_names.remove(&thread_id);
                    tracing::debug!("Isolate exited: {} (thread {})", isolate_id, thread_id);
                    let body = serde_json::json!({
                        "reason": "exited",
                        "threadId": thread_id,
                    });
                    self.send_event("thread", Some(body)).await;
                }

                // Clear active VM-tracked breakpoints and emit "unverified" events
                // for all desired breakpoints so the IDE shows grey dots during restart.
                let cleared = self.breakpoint_state.drain_all();
                if !cleared.is_empty() {
                    tracing::debug!(
                        "Isolate {} exited — cleared {} active breakpoints, marking desired as unverified",
                        isolate_id,
                        cleared.len(),
                    );
                }

                // Emit breakpoint changed (unverified) for every desired breakpoint.
                let unverified_events: Vec<serde_json::Value> = self
                    .desired_breakpoints
                    .values()
                    .flat_map(|bps| bps.iter())
                    .map(|dbp| {
                        serde_json::json!({
                            "reason": "changed",
                            "breakpoint": {
                                "id": dbp.dap_id,
                                "verified": false,
                            }
                        })
                    })
                    .collect();
                for body in unverified_events {
                    self.send_event("breakpoint", Some(body)).await;
                }
            }

            DebugEvent::Paused {
                isolate_id,
                reason,
                breakpoint_id,
                exception,
            } => {
                let thread_id = self.thread_map.get_or_create(&isolate_id);
                let reason_str = pause_reason_to_dap_str(&reason);
                tracing::debug!(
                    "Isolate paused: {} (thread {}), reason: {}",
                    isolate_id,
                    thread_id,
                    reason_str
                );

                // ── Conditional breakpoint / logpoint evaluation ──────────
                //
                // When the pause is at a breakpoint, check hit-condition and
                // expression condition before emitting `stopped`. If any
                // condition is not met, silently resume the isolate.
                //
                // If the breakpoint has a `log_message` (logpoint), and all
                // conditions pass, interpolate the message, emit a DAP `output`
                // event, and auto-resume **without** emitting `stopped`.
                if reason == PauseReason::Breakpoint {
                    if let Some(vm_bp_id) = &breakpoint_id {
                        // Increment hit count first (always, before any checks).
                        let hit_count = self
                            .breakpoint_state
                            .increment_hit_count(vm_bp_id)
                            .unwrap_or(1);

                        // Clone all condition fields out of the entry so we
                        // don't hold a borrow on `breakpoint_state` while
                        // calling async backend methods.
                        let (condition, hit_condition, log_message, bp_line, bp_uri) = self
                            .breakpoint_state
                            .lookup_by_vm_id(vm_bp_id)
                            .map(|e| {
                                (
                                    e.condition.clone(),
                                    e.hit_condition.clone(),
                                    e.log_message.clone(),
                                    e.line,
                                    e.uri.clone(),
                                )
                            })
                            .unwrap_or((None, None, None, None, String::new()));

                        // 1. Check hit condition (cheap — no RPC).
                        if let Some(hit_cond) = &hit_condition {
                            if !breakpoints::evaluate_hit_condition(hit_count, hit_cond) {
                                tracing::debug!(
                                    "Hit condition '{}' not met (count={}) — resuming silently",
                                    hit_cond,
                                    hit_count,
                                );
                                let _ = self.backend.resume(&isolate_id, None).await;
                                return;
                            }
                        }

                        // 2. Check expression condition (requires evaluateInFrame RPC).
                        if let Some(cond_expr) = &condition {
                            match self
                                .backend
                                .evaluate_in_frame(&isolate_id, 0, cond_expr)
                                .await
                            {
                                Ok(result) if breakpoints::is_truthy(&result) => {
                                    // Condition met — fall through.
                                }
                                Ok(_) => {
                                    // Condition evaluated to falsy — silently resume.
                                    tracing::debug!(
                                        "Condition '{}' evaluated to falsy — resuming silently",
                                        cond_expr,
                                    );
                                    let _ = self.backend.resume(&isolate_id, None).await;
                                    return;
                                }
                                Err(e) => {
                                    // Condition evaluation error — safe default: stop.
                                    tracing::warn!(
                                        "Conditional breakpoint evaluation failed for '{}': {} — stopping (safe default)",
                                        cond_expr,
                                        e,
                                    );
                                    // Fall through to emit stopped (or logpoint output).
                                }
                            }
                        }

                        // 3. Logpoint: if log_message is set, interpolate and emit output,
                        //    then auto-resume without emitting `stopped`.
                        if let Some(template) = log_message {
                            let output = self.interpolate_log_message(&isolate_id, &template).await;
                            tracing::debug!(
                                "Logpoint fired at {}:{:?} — output: {:?}",
                                bp_uri,
                                bp_line,
                                output,
                            );

                            // Resolve source location for the output event.
                            let source_path = dart_uri_to_path(&bp_uri);
                            let source_name =
                                bp_uri.rsplit('/').next().unwrap_or(&bp_uri).to_string();
                            let source = DapSource {
                                name: Some(source_name),
                                path: source_path,
                                source_reference: None,
                                presentation_hint: None,
                            };

                            let mut body = serde_json::json!({
                                "category": "console",
                                "output": output,
                                "source": serde_json::to_value(&source).unwrap_or_default(),
                            });
                            if let Some(line_no) = bp_line {
                                body["line"] = serde_json::json!(line_no);
                            }

                            self.send_event("output", Some(body)).await;
                            let _ = self.backend.resume(&isolate_id, None).await;
                            return;
                        }
                    }
                }

                // Track the paused isolate for evaluate context resolution.
                // Remove any prior entry for this isolate, then push to back
                // so that the most recently paused isolate is last.
                self.paused_isolates.retain(|id| id != &isolate_id);
                self.paused_isolates.push(isolate_id.clone());

                // Store the exception InstanceRef when paused at an exception.
                // Cleared on resume via on_resume(). Used by handle_scopes to
                // conditionally include an "Exceptions" scope.
                if reason == PauseReason::Exception {
                    if let Some(exc_value) = exception {
                        self.exception_refs.insert(
                            thread_id,
                            crate::adapter::ExceptionRef {
                                isolate_id: isolate_id.clone(),
                                instance_ref: exc_value,
                            },
                        );
                    }
                }

                let body = serde_json::json!({
                    "reason": reason_str,
                    "threadId": thread_id,
                    "allThreadsStopped": true,
                });
                self.send_event("stopped", Some(body)).await;
            }

            DebugEvent::Resumed { isolate_id } => {
                if let Some(thread_id) = self.thread_map.thread_id_for(&isolate_id) {
                    tracing::debug!("Isolate resumed: {} (thread {})", isolate_id, thread_id);
                    // Remove the isolate from the paused set.
                    self.paused_isolates.retain(|id| id != &isolate_id);
                    // Clear the exception ref for this thread — exception data is
                    // only valid while the isolate is stopped.
                    self.exception_refs.remove(&thread_id);
                    self.on_resume();
                    let body = serde_json::json!({
                        "threadId": thread_id,
                        "allThreadsContinued": true,
                    });
                    self.send_event("continued", Some(body)).await;
                }
            }

            DebugEvent::IsolateRunnable { isolate_id } => {
                // Re-apply all desired breakpoints to the new isolate.
                //
                // This is the correct trigger: the isolate is fully initialized
                // and can receive `addBreakpointWithScriptUri` calls.
                tracing::debug!(
                    "IsolateRunnable: re-applying desired breakpoints to {}",
                    isolate_id
                );

                // Collect desired breakpoints first (avoid borrow conflict).
                let to_apply: Vec<(String, crate::adapter::breakpoints::DesiredBreakpoint)> = self
                    .desired_breakpoints
                    .iter()
                    .flat_map(|(uri, bps)| {
                        bps.iter()
                            .map(|bp| (uri.clone(), bp.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();

                let mut reapplied_count = 0usize;
                for (uri, desired_bp) in &to_apply {
                    match self
                        .backend
                        .add_breakpoint(&isolate_id, uri, desired_bp.line, desired_bp.column)
                        .await
                    {
                        Ok(result) => {
                            let actual_line = result.line.or(Some(desired_bp.line));
                            let actual_col = result.column.or(desired_bp.column);
                            // Re-register the active breakpoint using the stable desired DAP ID.
                            self.breakpoint_state.insert_with_id(
                                desired_bp.dap_id,
                                result.vm_id.clone(),
                                uri.clone(),
                                actual_line,
                                actual_col,
                                result.resolved,
                                breakpoints::BreakpointCondition {
                                    condition: desired_bp.condition.clone(),
                                    hit_condition: desired_bp.hit_condition.clone(),
                                    log_message: desired_bp.log_message.clone(),
                                },
                            );
                            tracing::debug!(
                                "Re-applied breakpoint {}:{} → vm_id={} dap_id={}",
                                uri,
                                desired_bp.line,
                                result.vm_id,
                                desired_bp.dap_id,
                            );
                            // Emit verified event.
                            let body = serde_json::json!({
                                "reason": "changed",
                                "breakpoint": {
                                    "id": desired_bp.dap_id,
                                    "verified": result.resolved,
                                    "line": actual_line,
                                }
                            });
                            self.send_event("breakpoint", Some(body)).await;
                            reapplied_count += 1;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to re-apply breakpoint {}:{} on new isolate: {}",
                                uri,
                                desired_bp.line,
                                e,
                            );
                            // Emit unverified event with error message.
                            let body = serde_json::json!({
                                "reason": "changed",
                                "breakpoint": {
                                    "id": desired_bp.dap_id,
                                    "verified": false,
                                    "message": format!("Could not re-apply breakpoint: {}", e),
                                }
                            });
                            self.send_event("breakpoint", Some(body)).await;
                        }
                    }
                }

                // Re-apply exception pause mode to the new isolate.
                if self.exception_mode != DapExceptionPauseMode::None {
                    let _ = self
                        .backend
                        .set_exception_pause_mode(&isolate_id, self.exception_mode)
                        .await;
                    tracing::debug!(
                        "Re-applied exception pause mode {:?} to new isolate {}",
                        self.exception_mode,
                        isolate_id,
                    );
                }

                tracing::debug!(
                    "IsolateRunnable: re-applied {} of {} desired breakpoints to {}",
                    reapplied_count,
                    to_apply.len(),
                    isolate_id,
                );
            }

            DebugEvent::BreakpointResolved {
                vm_breakpoint_id,
                line,
                column,
            } => {
                tracing::debug!("Breakpoint resolved: {}", vm_breakpoint_id);
                if let Some(bp) =
                    self.breakpoint_state
                        .resolve_breakpoint(&vm_breakpoint_id, line, column)
                {
                    let body = serde_json::json!({
                        "reason": "changed",
                        "breakpoint": {
                            "id": bp.dap_id,
                            "verified": true,
                            "line": bp.line,
                            "column": bp.column,
                        },
                    });
                    self.send_event("breakpoint", Some(body)).await;
                }
            }

            DebugEvent::AppExited { exit_code } => {
                tracing::debug!("App exited with code: {:?}", exit_code);

                // Mark the adapter as disconnected so subsequent requests return
                // a structured error rather than attempting backend calls.
                self.vm_disconnected = true;

                let body = serde_json::json!({
                    "exitCode": exit_code.unwrap_or(0),
                });
                self.send_event("exited", Some(body)).await;
                self.send_event("terminated", None).await;
            }

            DebugEvent::LogOutput {
                message,
                level,
                source_uri,
                line,
            } => {
                let category = log_level_to_category(&level);

                // Ensure message ends with newline (DAP convention for output events).
                let output = if message.ends_with('\n') {
                    message
                } else {
                    format!("{}\n", message)
                };

                let mut body = serde_json::json!({
                    "category": category,
                    "output": output,
                });

                // Resolve source location for clickable links in IDE consoles.
                if let Some(uri) = source_uri {
                    let path = dart_uri_to_path(&uri);
                    let name = uri.rsplit('/').next().unwrap_or(&uri).to_string();
                    let source = DapSource {
                        name: Some(name),
                        path,
                        source_reference: None,
                        presentation_hint: None,
                    };
                    body["source"] = serde_json::to_value(&source).unwrap_or_default();
                    if let Some(line_number) = line {
                        body["line"] = serde_json::json!(line_number);
                    }
                }

                self.send_event("output", Some(body)).await;
            }

            DebugEvent::AppStarted => {
                // The Flutter app is fully started and ready for interaction.
                // Emit the flutter.appStarted custom DAP event with an empty body,
                // as per the Flutter DAP convention.
                tracing::debug!("Emitting flutter.appStarted event");
                self.send_event("flutter.appStarted", Some(serde_json::json!({})))
                    .await;
            }
        }
    }

    /// Emit a plain text `output` event to the IDE debug console.
    ///
    /// This is a convenience wrapper for lifecycle messages (e.g., "Attached
    /// to VM Service", "Hot reload completed"). The `category` must be one of
    /// `"console"`, `"stdout"`, or `"stderr"`.
    ///
    /// The output text is sent as-is; append `'\n'` to the message if a
    /// newline separator is desired.
    pub async fn emit_output(&self, category: &str, output: &str) {
        let body = serde_json::json!({
            "category": category,
            "output": output,
        });
        self.send_event("output", Some(body)).await;
    }

    /// Interpolate a logpoint message template against the current frame.
    ///
    /// Parses `template` with [`breakpoints::parse_log_message`] and evaluates
    /// each `{expression}` segment via `evaluateInFrame` at frame index 0 (the
    /// top of the call stack). Evaluation errors are replaced with `"<error>"`
    /// so that the rest of the message is still emitted.
    ///
    /// The returned string always ends with `'\n'` (DAP convention for output
    /// events).
    ///
    /// # Performance note
    ///
    /// Each `{expression}` placeholder requires one `evaluateInFrame` RPC
    /// round-trip. For hot code paths with many placeholders this may add
    /// noticeable latency per logpoint hit.
    async fn interpolate_log_message(&self, isolate_id: &str, template: &str) -> String {
        let segments = breakpoints::parse_log_message(template);
        let mut result = String::new();

        for segment in &segments {
            match segment {
                breakpoints::LogSegment::Literal(text) => {
                    result.push_str(text);
                }
                breakpoints::LogSegment::Expression(expr) => {
                    let evaluated = self.backend.evaluate_in_frame(isolate_id, 0, expr).await;

                    match evaluated {
                        Ok(val) => {
                            // Extract the human-readable string representation.
                            let text = val
                                .get("valueAsString")
                                .and_then(|v| v.as_str())
                                .unwrap_or_else(|| {
                                    // Fall back to the kind string for non-primitive types.
                                    val.get("kind")
                                        .and_then(|k| k.as_str())
                                        .unwrap_or("<unknown>")
                                });
                            result.push_str(text);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Logpoint expression '{}' evaluation failed: {} — substituting <error>",
                                expr,
                                e,
                            );
                            result.push_str("<error>");
                        }
                    }
                }
            }
        }

        // Always end with newline (DAP output event convention).
        if !result.ends_with('\n') {
            result.push('\n');
        }

        result
    }

    /// Invalidate per-stop state (variable references and frame IDs).
    ///
    /// Must be called whenever the debuggee resumes. Variable references and
    /// frame IDs are only valid while the debuggee is stopped; they must be
    /// rebuilt from scratch on the next stop.
    ///
    /// Source references are **not** cleared here — they persist across
    /// stop/resume transitions and are only invalidated on hot restart via
    /// [`DapAdapter::on_hot_restart`].
    pub fn on_resume(&mut self) {
        self.var_store.reset();
        self.frame_store.reset();
    }

    /// Invalidate source references after a hot restart.
    ///
    /// Hot restart creates a new Dart isolate, making all previously allocated
    /// source reference IDs invalid (the new isolate has different script IDs).
    /// Clearing the store prevents stale source content from being served.
    ///
    /// Variable references and frame IDs are also reset here because hot restart
    /// is equivalent to a fresh start.
    ///
    /// **Note**: `desired_breakpoints` are intentionally **not** cleared here.
    /// They survive hot restart and are re-applied on `IsolateRunnable`.
    pub fn on_hot_restart(&mut self) {
        self.source_reference_store.clear();
        self.var_store.reset();
        self.frame_store.reset();
        // Active VM-tracked breakpoints are cleared here. Re-application happens
        // on IsolateRunnable via handle_debug_event.
        self.breakpoint_state.drain_all();
    }

    /// Send a DAP event to the client via the event channel.
    ///
    /// Made `pub(crate)` so that handler methods in sibling submodules (e.g.,
    /// `handlers.rs`) and the parent `mod.rs` can emit events without going
    /// through a separate channel abstraction.
    pub(crate) async fn send_event(&self, event: &str, body: Option<serde_json::Value>) {
        let dap_event = DapEvent {
            seq: 0, // Sequence number is assigned by the session writer.
            event: event.to_string(),
            body,
        };
        // Ignore send errors — the channel closing means the session ended.
        let _ = self.event_tx.send(DapMessage::Event(dap_event)).await;
    }
}

/// Convert a [`PauseReason`] to the DAP `stopped` event reason string.
pub(crate) fn pause_reason_to_dap_str(reason: &PauseReason) -> &'static str {
    match reason {
        PauseReason::Breakpoint => "breakpoint",
        PauseReason::Exception => "exception",
        PauseReason::Step => "step",
        PauseReason::Interrupted => "pause",
        PauseReason::Entry => "entry",
        PauseReason::Exit => "exit",
    }
}
