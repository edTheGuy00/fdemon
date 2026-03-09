//! # Variable & Scope Handling
//!
//! DapAdapter methods for stack traces, scopes, and variable inspection.

use crate::adapter::backend::DebugBackend;
use crate::adapter::handlers::parse_args;
use crate::adapter::stack::{
    extract_line_column, extract_source, FrameRef, ScopeKind, VariableRef,
};
use crate::adapter::types::MAX_VARIABLES_PER_REQUEST;
use crate::adapter::DapAdapter;
use crate::protocol::types::{
    DapScope, DapStackFrame, DapVariable, ScopesArguments, StackTraceArguments, VariablesArguments,
};
use crate::{DapRequest, DapResponse};

impl<B: DebugBackend> DapAdapter<B> {
    /// Handle the `stackTrace` request.
    ///
    /// Returns the current call stack for a paused isolate, mapped from VM
    /// Service frame objects to [`DapStackFrame`] objects. Each frame is
    /// allocated a unique frame ID via [`FrameStore`] for later `scopes`
    /// and `variables` requests.
    ///
    /// # Pagination
    ///
    /// The `startFrame` and `levels` arguments are respected so that Zed (which
    /// sends `supportsDelayedStackTraceLoading: true`) can fetch frames lazily.
    ///
    /// # Async frames
    ///
    /// Dart's VM reports async suspension markers as frames with
    /// `kind: "AsyncSuspensionMarker"`. These are rendered with name
    /// `"<asynchronous gap>"` and `presentation_hint: "label"` to serve as
    /// visual separators, matching the behavior of the official Dart debugger.
    pub(super) async fn handle_stack_trace(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: stackTrace");

        let args = match parse_args::<StackTraceArguments>(request) {
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

        // Clamp the `levels` argument for the VM Service call.
        let limit = args.levels.map(|l| l as i32);

        let stack_json = match self.backend.get_stack(&isolate_id, limit).await {
            Ok(v) => v,
            Err(e) => return DapResponse::error(request, format!("Failed to get stack: {e}")),
        };

        let frames: &[serde_json::Value] = stack_json
            .get("frames")
            .and_then(|f| f.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        let total_frames = frames.len();
        let start_frame = args.start_frame.unwrap_or(0) as usize;

        let mut dap_frames: Vec<DapStackFrame> = Vec::new();

        for (i, frame) in frames.iter().enumerate().skip(start_frame) {
            let frame_index = i as i32;

            // Allocate a stable DAP frame ID for this frame.
            let frame_id = self.frame_store.allocate(FrameRef {
                isolate_id: isolate_id.clone(),
                frame_index,
            });

            let kind = frame.get("kind").and_then(|k| k.as_str()).unwrap_or("");

            // Async suspension markers are visual separators, not real frames.
            let (name, presentation_hint) = if kind == "AsyncSuspensionMarker" {
                ("<asynchronous gap>".to_string(), Some("label".to_string()))
            } else {
                let code_name = frame
                    .get("code")
                    .and_then(|c| c.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("<unknown>")
                    .to_string();
                (code_name, None)
            };

            let source = extract_source(frame);
            let (line, column) = extract_line_column(frame);

            dap_frames.push(DapStackFrame {
                id: frame_id,
                name,
                source,
                line: line.unwrap_or(0) as i64,
                column: column.unwrap_or(0) as i64,
                end_line: None,
                end_column: None,
                presentation_hint,
            });
        }

        let body = serde_json::json!({
            "stackFrames": dap_frames,
            "totalFrames": total_frames,
        });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `scopes` request.
    ///
    /// Returns the scopes (variable groupings) for a given stack frame. This
    /// handler is **synchronous** — it only allocates variable references for
    /// the scopes without making VM Service calls. The expensive work happens
    /// when the client later calls `variables` with each reference.
    ///
    /// # Scopes Returned
    ///
    /// - **Locals** (`expensive: false`) — local variables visible in this frame
    /// - **Globals** (`expensive: true`) — module-level variables (can be large)
    ///
    /// # Helix Compatibility
    ///
    /// Helix sets `supportsVariablePaging: false`, so the adapter must return
    /// the complete variable list when `variables` is called. The paging
    /// parameters (`start`, `count`) from `VariablesArguments` are ignored.
    pub(super) async fn handle_scopes(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: scopes");

        let args = match parse_args::<ScopesArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        let frame_ref = match self.frame_store.lookup(args.frame_id) {
            Some(fr) => fr.clone(),
            None => {
                return DapResponse::error(
                    request,
                    format!(
                        "Invalid frame ID {} (stale or unknown — did the program resume?)",
                        args.frame_id
                    ),
                )
            }
        };

        // Allocate a variable reference for the Locals scope.
        let locals_ref = self.var_store.allocate(VariableRef::Scope {
            frame_index: frame_ref.frame_index,
            scope_kind: ScopeKind::Locals,
        });

        // Allocate a variable reference for the Globals scope.
        let globals_ref = self.var_store.allocate(VariableRef::Scope {
            frame_index: frame_ref.frame_index,
            scope_kind: ScopeKind::Globals,
        });

        let scopes = vec![
            DapScope {
                name: "Locals".to_string(),
                presentation_hint: Some("locals".to_string()),
                variables_reference: locals_ref,
                named_variables: None,
                indexed_variables: None,
                expensive: false,
            },
            DapScope {
                name: "Globals".to_string(),
                // "globals" is not a standard DAP hint, but it is informative for
                // clients that support custom hints.
                presentation_hint: Some("globals".to_string()),
                variables_reference: globals_ref,
                named_variables: None,
                indexed_variables: None,
                expensive: true, // Globals can be large — flag for lazy loading.
            },
        ];

        let body = serde_json::json!({ "scopes": scopes });
        DapResponse::success(request, Some(body))
    }

    /// Handle the `variables` request.
    ///
    /// Resolves a variable reference (from a prior `scopes` or `variables`
    /// response) to a list of DAP variables. Two kinds of reference are
    /// supported:
    ///
    /// - [`VariableRef::Scope`] — fetch the frame's locals from the VM Service
    ///   and map each `InstanceRef` to a [`DapVariable`].
    /// - [`VariableRef::Object`] — call `getObject` on the VM Service and
    ///   expand the object's children (list elements, map entries, or fields).
    ///
    /// Stale or unknown references (i.e., those from a previous stop that were
    /// invalidated by [`DapAdapter::on_resume`]) return a clear error.
    pub(super) async fn handle_variables(&mut self, request: &DapRequest) -> DapResponse {
        tracing::debug!("DAP adapter: variables");

        let args = match parse_args::<VariablesArguments>(request) {
            Ok(a) => a,
            Err(e) => return DapResponse::error(request, e),
        };

        // Look up what this reference points to.
        let var_ref = match self.var_store.lookup(args.variables_reference) {
            Some(vr) => vr.clone(),
            None => {
                return DapResponse::error(
                    request,
                    format!(
                    "Invalid variables reference {} (stale or unknown — did the program resume?)",
                    args.variables_reference
                ),
                )
            }
        };

        // Apply rate limiting: cap the requested count at MAX_VARIABLES_PER_REQUEST.
        // The `start` offset is passed through as-is to the backend (pagination
        // is transparent to the IDE — the backend handles offset and count together).
        let capped_count = args
            .count
            .map(|c| c.min(MAX_VARIABLES_PER_REQUEST as i64))
            .unwrap_or(MAX_VARIABLES_PER_REQUEST as i64);

        let variables = match var_ref {
            VariableRef::Scope {
                frame_index,
                scope_kind,
            } => {
                // Scope variables: the backend returns the full list; we apply
                // start/count pagination here since the VM does not paginate scopes.
                let all = self.get_scope_variables(frame_index, scope_kind).await;
                match all {
                    Ok(vars) => {
                        let start = args.start.unwrap_or(0) as usize;
                        let paged: Vec<_> = vars
                            .into_iter()
                            .skip(start)
                            .take(capped_count as usize)
                            .collect();
                        Ok(paged)
                    }
                    Err(e) => Err(e),
                }
            }
            VariableRef::Object {
                isolate_id,
                object_id,
            } => {
                // Object expansion: pass start/count to the backend so the VM
                // Service returns only the requested slice (e.g., list elements).
                self.expand_object(
                    &isolate_id.clone(),
                    &object_id.clone(),
                    args.start,
                    Some(capped_count),
                )
                .await
            }
        };

        match variables {
            Ok(vars) => {
                let body = serde_json::json!({ "variables": vars });
                DapResponse::success(request, Some(body))
            }
            Err(e) => DapResponse::error(request, format!("Failed to get variables: {e}")),
        }
    }

    /// Fetch the variables for a scope (locals or globals) from the VM Service.
    ///
    /// For `Locals`: calls `get_stack` on the backend and maps each frame
    /// variable's `InstanceRef` to a [`DapVariable`].
    ///
    /// For `Globals`: returns an empty list in Phase 3 (globals are expensive
    /// and deferred to Phase 4).
    async fn get_scope_variables(
        &mut self,
        frame_index: i32,
        scope_kind: ScopeKind,
    ) -> Result<Vec<DapVariable>, String> {
        match scope_kind {
            ScopeKind::Locals => {
                // Look up the isolate ID for this frame.
                let isolate_id = self
                    .frame_store
                    .lookup_by_index(frame_index)
                    .map(|fr| fr.isolate_id.clone())
                    .ok_or_else(|| {
                        format!("Frame index {} not found in frame store", frame_index)
                    })?;

                // Fetch the stack up to frame_index + 1 to include our frame.
                let stack = self
                    .backend
                    .get_stack(&isolate_id, Some(frame_index + 1))
                    .await
                    .map_err(|e| e.to_string())?;

                let frames = stack
                    .get("frames")
                    .and_then(|f| f.as_array())
                    .map(|a| a.as_slice())
                    .unwrap_or(&[]);

                let frame = frames
                    .get(frame_index as usize)
                    .ok_or_else(|| format!("Frame index {} out of bounds", frame_index))?;

                let vars: Vec<serde_json::Value> = frame
                    .get("vars")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();

                let isolate_id_clone = isolate_id.clone();
                let mut result = Vec::with_capacity(vars.len());
                for var in &vars {
                    let name = var
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let value = var.get("value").cloned().unwrap_or(serde_json::Value::Null);
                    result.push(self.instance_ref_to_variable(&name, &value, &isolate_id_clone));
                }
                Ok(result)
            }
            ScopeKind::Globals => {
                // Globals are expensive — return empty for now.
                // Phase 4 will add full support via the isolate's libraries.
                Ok(Vec::new())
            }
        }
    }

    /// Convert a VM Service `InstanceRef` JSON value to a DAP [`DapVariable`].
    ///
    /// Primitives (`Null`, `Bool`, `Int`, `Double`, `String`) are rendered
    /// inline with `variables_reference: 0` (no expansion). Complex types
    /// (collections and plain instances) are allocated a variable reference
    /// that the IDE can use to drill in further.
    pub(super) fn instance_ref_to_variable(
        &mut self,
        name: &str,
        instance_ref: &serde_json::Value,
        isolate_id: &str,
    ) -> DapVariable {
        let kind = instance_ref
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("");
        let class_name = instance_ref
            .get("class")
            .and_then(|c| c.get("name"))
            .and_then(|n| n.as_str());
        let value_as_string = instance_ref.get("valueAsString").and_then(|v| v.as_str());
        let obj_id = instance_ref.get("id").and_then(|i| i.as_str());

        match kind {
            // ── Primitives: inline value, no expansion ───────────────────────
            "Null" => DapVariable {
                name: name.to_string(),
                value: "null".to_string(),
                type_field: Some("Null".to_string()),
                variables_reference: 0,
                ..Default::default()
            },

            "Bool" => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("false").to_string(),
                type_field: Some("bool".to_string()),
                variables_reference: 0,
                ..Default::default()
            },

            "Int" | "Double" => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("0").to_string(),
                type_field: Some(kind.to_lowercase()),
                variables_reference: 0,
                ..Default::default()
            },

            "String" => {
                let value = value_as_string
                    .map(|s| format!("\"{}\"", s))
                    .unwrap_or_else(|| "\"\"".to_string());
                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some("String".to_string()),
                    variables_reference: 0,
                    ..Default::default()
                }
            }

            // ── Collections: expandable ──────────────────────────────────────
            "List" | "Map" | "Set" | "Uint8ClampedList" | "Uint8List" | "Int32List"
            | "Float64List" => {
                let length = instance_ref
                    .get("length")
                    .and_then(|l| l.as_i64())
                    .unwrap_or(0);
                let type_name = class_name.unwrap_or(kind);
                let value = format!("{} (length: {})", type_name, length);

                let var_ref = if let Some(id) = obj_id {
                    self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    })
                } else {
                    0
                };

                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some(type_name.to_string()),
                    variables_reference: var_ref,
                    indexed_variables: Some(length),
                    ..Default::default()
                }
            }

            // ── Plain instances: expandable via fields ───────────────────────
            "PlainInstance" | "Closure" | "RegExp" | "Type" | "StackTrace" => {
                let type_name = class_name.unwrap_or(kind);
                let value = value_as_string
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{} instance", type_name));

                let var_ref = if let Some(id) = obj_id {
                    self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    })
                } else {
                    0
                };

                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some(type_name.to_string()),
                    variables_reference: var_ref,
                    ..Default::default()
                }
            }

            // ── Fallback ─────────────────────────────────────────────────────
            _ => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("<unknown>").to_string(),
                type_field: class_name.map(|s| s.to_string()),
                variables_reference: 0,
                ..Default::default()
            },
        }
    }

    /// Expand a VM Service object into a list of [`DapVariable`] children.
    ///
    /// Fetches the full object via `get_object` and dispatches based on the
    /// object's `kind`:
    ///
    /// - `List` / typed arrays — indexed elements `[0]`, `[1]`, …
    /// - `Map` — keyed entries `[key]`, …
    /// - `PlainInstance` and others — named fields
    ///
    /// The `start` and `count` paging parameters are forwarded to the VM
    /// Service so that large collections can be fetched in chunks.
    async fn expand_object(
        &mut self,
        isolate_id: &str,
        object_id: &str,
        start: Option<i64>,
        count: Option<i64>,
    ) -> Result<Vec<DapVariable>, String> {
        let obj = self
            .backend
            .get_object(isolate_id, object_id, start, count)
            .await
            .map_err(|e| e.to_string())?;

        let obj_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match obj_type {
            "Instance" => {
                let kind = obj.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                match kind {
                    "List" | "Uint8List" | "Uint8ClampedList" | "Int32List" | "Float64List" => {
                        // Expand list elements.
                        let elements: Vec<serde_json::Value> = obj
                            .get("elements")
                            .and_then(|e| e.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let offset = start.unwrap_or(0);
                        let isolate_id = isolate_id.to_string();

                        let mut result = Vec::with_capacity(elements.len());
                        for (i, elem) in elements.iter().enumerate() {
                            let index = offset + i as i64;
                            let elem_name = format!("[{}]", index);
                            result.push(self.instance_ref_to_variable(
                                &elem_name,
                                elem,
                                &isolate_id,
                            ));
                        }
                        Ok(result)
                    }

                    "Map" => {
                        // Expand map associations.
                        let associations: Vec<serde_json::Value> = obj
                            .get("associations")
                            .and_then(|a| a.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let isolate_id = isolate_id.to_string();

                        let mut result = Vec::with_capacity(associations.len());
                        for assoc in &associations {
                            let key = assoc
                                .get("key")
                                .and_then(|k| k.get("valueAsString"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let value = assoc
                                .get("value")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let entry_name = format!("[{}]", key);
                            result.push(self.instance_ref_to_variable(
                                &entry_name,
                                &value,
                                &isolate_id,
                            ));
                        }
                        Ok(result)
                    }

                    _ => {
                        // Expand instance fields.
                        let fields: Vec<serde_json::Value> = obj
                            .get("fields")
                            .and_then(|f| f.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let isolate_id = isolate_id.to_string();

                        let mut result = Vec::with_capacity(fields.len());
                        for field in &fields {
                            let name = field
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("?")
                                .to_string();
                            let value = field
                                .get("value")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            result.push(self.instance_ref_to_variable(&name, &value, &isolate_id));
                        }
                        Ok(result)
                    }
                }
            }
            _ => Ok(Vec::new()),
        }
    }
}
