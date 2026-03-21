//! # Variable & Scope Handling
//!
//! DapAdapter methods for stack traces, scopes, and variable inspection.

use crate::adapter::backend::DebugBackend;
use crate::adapter::handlers::{parse_args, with_timeout};
use crate::adapter::stack::{
    extract_line_column, extract_source_with_store, FrameRef, ScopeKind, VariableRef,
};
use crate::adapter::types::MAX_VARIABLES_PER_REQUEST;
use crate::adapter::DapAdapter;
use crate::protocol::types::{
    DapScope, DapStackFrame, DapVariable, DapVariablePresentationHint, ScopesArguments,
    StackTraceArguments, VariablesArguments,
};
use crate::{DapRequest, DapResponse};

/// Maximum number of getter evaluations per object expansion.
///
/// Prevents extremely large class hierarchies from hanging the debugger by
/// limiting how many getters are collected and evaluated when expanding a
/// `PlainInstance` object.
const MAX_GETTER_EVALUATIONS: usize = 50;

/// Maximum depth when traversing the superclass chain for getter collection.
///
/// Prevents infinite loops in malformed class hierarchies (e.g., circular
/// super-class references) by stopping traversal at this depth.
const MAX_SUPERCLASS_DEPTH: usize = 10;

/// Timeout for a single getter evaluation.
///
/// Matches the Dart DDS adapter's 1-second timeout for getter evaluation.
/// If the VM Service does not respond within this duration, the getter value
/// is shown as `"<timed out>"` without crashing the adapter.
const GETTER_EVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// Internal getter names that are filtered out during getter collection.
///
/// These are VM-internal getters that are not useful to show in the debugger:
/// - `_identityHashCode`: VM-internal object identity hash, always present.
/// - `hashCode`: only meaningful for primitives; the default `Object.hashCode`
///   is usually `_identityHashCode`, adding noise without value.
/// - `runtimeType`: returns the Dart `Type` object; rarely useful in debugger.
const FILTERED_GETTER_NAMES: &[&str] = &["_identityHashCode", "hashCode", "runtimeType"];

/// Timeout for a single `toString()` evaluation when enriching variable display values.
///
/// A 1-second timeout is critical: some `toString()` implementations in user
/// code can be expensive or buggy. The variables panel must never hang because
/// of a bad `toString()`. On timeout, the variable displays just the class name.
const TO_STRING_EVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

/// Maximum total time for all `toString()` calls in a single variables response.
///
/// Even with a 1-second per-call timeout, 20 `PlainInstance` variables would
/// take up to 20 seconds sequentially. This budget caps the entire enrichment
/// pass so the IDE panel always responds within a bounded time.
///
/// When the budget is exhausted, remaining candidates are skipped and their
/// variables keep the unenriched class-name display value.
const TO_STRING_TOTAL_BUDGET: std::time::Duration = std::time::Duration::from_secs(3);

/// Maximum total time for all getter evaluations on a single object expansion.
///
/// Even with a 1-second per-getter timeout, an object with 50 getters could
/// take up to 50 seconds. This budget caps the total getter evaluation time so
/// the variables panel remains responsive.
///
/// When the budget is exhausted, remaining getters are added as lazy (unexpanded)
/// items so the user can still expand individual getters on demand.
const GETTER_EVAL_TOTAL_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// Instance kinds for which `toString()` enrichment is applied.
///
/// Primitives, collections, closures, and sentinels already show their values
/// inline; only complex object kinds benefit from calling `toString()`.
const TO_STRING_KINDS: &[&str] = &["PlainInstance", "RegExp", "StackTrace", "WeakReference"];

/// Metadata for a variable that requires `toString()` enrichment.
///
/// Collected during `get_scope_variables` and then used in the enrichment
/// pass to call `evaluate(obj_id, "toString()")` on the VM Service.
struct ToStringCandidate {
    /// Index of the variable in the result vector that needs enrichment.
    var_index: usize,
    /// The Dart VM Service isolate ID for the `evaluate` call.
    isolate_id: String,
    /// The object ID to evaluate `toString()` on.
    object_id: String,
    /// The class display name (used to suppress default Dart toString output).
    class_name: String,
}

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

        let stack_json = match with_timeout(self.backend.get_stack(&isolate_id, limit)).await {
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

            // Track the first async suspension marker index for restartFrame boundary checks.
            // Only record the first one encountered (lowest-index boundary).
            if kind == "AsyncSuspensionMarker" && self.first_async_marker_index.is_none() {
                self.first_async_marker_index = Some(frame_index);
            }

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

            let source = extract_source_with_store(
                frame,
                &mut self.source_reference_store,
                &isolate_id,
                None, // project_root not available in DapAdapter; source reference allocation still works
            );
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

        let mut scopes = vec![
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

        // Conditionally add an "Exceptions" scope when the thread is paused
        // at an exception. The scope contains a single variable (the exception
        // object) that can be expanded to inspect its fields.
        let thread_id = self.thread_map.thread_id_for(&frame_ref.isolate_id);
        if let Some(tid) = thread_id {
            if self.exception_refs.contains_key(&tid) {
                let exc_ref = self.var_store.allocate(VariableRef::Scope {
                    frame_index: frame_ref.frame_index,
                    scope_kind: ScopeKind::Exceptions,
                });
                scopes.push(DapScope {
                    name: "Exceptions".to_string(),
                    presentation_hint: Some("locals".to_string()),
                    variables_reference: exc_ref,
                    named_variables: None,
                    indexed_variables: None,
                    expensive: false,
                });
            }
        }

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
                // Look up the evaluateName for this variable reference so that
                // expand_object can construct child expressions (e.g., obj.field,
                // list[0]).
                let parent_eval_name = self
                    .evaluate_name_map
                    .get(&args.variables_reference)
                    .cloned();
                // Object expansion: pass start/count to the backend so the VM
                // Service returns only the requested slice (e.g., list elements).
                self.expand_object(
                    &isolate_id,
                    &object_id,
                    args.start,
                    Some(capped_count),
                    parent_eval_name.as_deref(),
                )
                .await
            }
            VariableRef::GetterEval {
                isolate_id,
                instance_id,
                getter_name,
            } => {
                // Lazy getter evaluation: triggered when the user explicitly
                // expands a getter that was deferred with `evaluateGettersInDebugViews == false`.
                self.evaluate_lazy_getter(&isolate_id, &instance_id, &getter_name)
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
    /// For `Globals`: enumerates library-level static fields from the current
    /// frame's library. If the frame has no library context (e.g., async gap
    /// frames), falls back to the isolate's root library.
    async fn get_scope_variables(
        &mut self,
        frame_index: i32,
        scope_kind: ScopeKind,
    ) -> Result<Vec<DapVariable>, String> {
        match scope_kind {
            ScopeKind::Exceptions => {
                // Look up the isolate ID and derive the thread ID for this frame.
                let isolate_id = self
                    .frame_store
                    .lookup_by_index(frame_index)
                    .map(|fr| fr.isolate_id.clone())
                    .ok_or_else(|| {
                        format!("Frame index {} not found in frame store", frame_index)
                    })?;

                let thread_id = self
                    .thread_map
                    .thread_id_for(&isolate_id)
                    .ok_or_else(|| format!("No thread found for isolate '{}'", isolate_id))?;

                if let Some(exc) = self.exception_refs.get(&thread_id) {
                    // Extract the class name from the exception InstanceRef.
                    // The VM wire format may use "classRef" (serde camelCase) or
                    // "class" (raw get_object path); try both for resilience.
                    let class_name = exc
                        .instance_ref
                        .get("classRef")
                        .or_else(|| exc.instance_ref.get("class"))
                        .and_then(|c| c.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("Exception")
                        .to_string();
                    let instance_ref = exc.instance_ref.clone();
                    let isolate_id_clone = exc.isolate_id.clone();
                    let var = self.instance_ref_to_variable_with_eval_name(
                        &class_name,
                        &instance_ref,
                        &isolate_id_clone,
                        Some("$_threadException"),
                    );
                    let mut result = vec![var];
                    // Enrich exception with toString() if enabled.
                    if self.evaluate_to_string_in_debug_views {
                        let candidates: Vec<ToStringCandidate> =
                            to_string_candidate(0, &isolate_id_clone, &instance_ref)
                                .into_iter()
                                .collect();
                        self.enrich_with_to_string(&mut result, candidates).await;
                    }
                    Ok(result)
                } else {
                    Ok(Vec::new())
                }
            }
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
                let stack =
                    with_timeout(self.backend.get_stack(&isolate_id, Some(frame_index + 1)))
                        .await?;

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
                let mut candidates: Vec<ToStringCandidate> = Vec::new();
                for var in &vars {
                    let name = var
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let value = var.get("value").cloned().unwrap_or(serde_json::Value::Null);
                    // Collect toString() enrichment candidate before converting.
                    if self.evaluate_to_string_in_debug_views {
                        if let Some(candidate) =
                            to_string_candidate(result.len(), &isolate_id_clone, &value)
                        {
                            candidates.push(candidate);
                        }
                    }
                    // Pass the variable name as its evaluateName so the IDE can
                    // use it in "Add to Watch" and nested expression drill-down.
                    result.push(self.instance_ref_to_variable_with_eval_name(
                        &name,
                        &value,
                        &isolate_id_clone,
                        Some(&name),
                    ));
                }
                // Second pass: enrich PlainInstance variables with toString().
                self.enrich_with_to_string(&mut result, candidates).await;
                Ok(result)
            }
            ScopeKind::Globals => self.get_globals_variables(frame_index).await,
        }
    }

    /// Enrich a list of variables with `toString()` display values.
    ///
    /// For each [`ToStringCandidate`] in `candidates`, calls `evaluate` on the
    /// VM Service with the expression `"toString()"` and a
    /// [`TO_STRING_EVAL_TIMEOUT`] timeout. If the result is non-empty and is
    /// not the default Dart `"Instance of 'ClassName'"` output, it is appended
    /// to the variable's display value: `"MyClass (custom string repr)"`.
    ///
    /// Errors and timeouts silently fall back to the current display value.
    /// toString calls are made sequentially to avoid overwhelming the VM.
    ///
    /// A [`TO_STRING_TOTAL_BUDGET`] caps the entire enrichment pass: if the
    /// budget is exhausted before all candidates are processed, remaining
    /// candidates are skipped and their variables keep the unenriched display.
    ///
    /// This method is a no-op when `self.evaluate_to_string_in_debug_views` is
    /// `false` (caller is responsible for checking before collecting candidates).
    async fn enrich_with_to_string(
        &self,
        variables: &mut [DapVariable],
        candidates: Vec<ToStringCandidate>,
    ) {
        let deadline = tokio::time::Instant::now() + TO_STRING_TOTAL_BUDGET;
        let total = candidates.len();

        for (idx, candidate) in candidates.into_iter().enumerate() {
            // Check the total budget before each call.
            if tokio::time::Instant::now() >= deadline {
                tracing::debug!(
                    "toString enrichment budget exhausted ({:?}), skipping remaining {} of {} candidates",
                    TO_STRING_TOTAL_BUDGET,
                    total - idx,
                    total,
                );
                break;
            }

            let result = tokio::time::timeout(
                TO_STRING_EVAL_TIMEOUT,
                self.backend
                    .evaluate(&candidate.isolate_id, &candidate.object_id, "toString()"),
            )
            .await;

            let Ok(Ok(ref response)) = result else {
                // Timeout or backend error — silently keep the existing value.
                continue;
            };

            let str_val = response
                .get("valueAsString")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Suppress the default Dart toString() output.
            let default_output = format!("Instance of '{}'", candidate.class_name);
            if str_val.is_empty() || str_val == default_output {
                continue;
            }

            // Append the useful toString() output to the display value.
            if let Some(var) = variables.get_mut(candidate.var_index) {
                var.value = format!("{} ({})", candidate.class_name, str_val);
            }
        }
    }

    /// Enumerate library-level static fields for the Globals scope.
    ///
    /// Attempts to find the library for the current frame by inspecting
    /// `frame.code.owner`. If the owner is a `LibraryRef`, it is used directly.
    /// If the owner is a `ClassRef`, the class's `library` field is used.
    /// If neither is available (e.g., closure or async gap frames), falls back
    /// to the isolate's `rootLib`.
    ///
    /// Each field in the library's `variables` array is resolved to its
    /// `staticValue` via `get_object`, then converted to a [`DapVariable`] with
    /// `presentationHint.attributes: ["static"]`. Uninitialized fields (no
    /// `staticValue`) display `"<not initialized>"`.
    ///
    /// Private fields (names starting with `_`) are included with
    /// `presentationHint.visibility: "private"`. `const` fields carry
    /// `["static", "readOnly", "constant"]` attributes.
    async fn get_globals_variables(
        &mut self,
        frame_index: i32,
    ) -> Result<Vec<DapVariable>, String> {
        // Step 1: resolve the isolate ID.
        let isolate_id = self
            .frame_store
            .lookup_by_index(frame_index)
            .map(|fr| fr.isolate_id.clone())
            .ok_or_else(|| format!("Frame index {} not found in frame store", frame_index))?;

        // Step 2: determine the library ID from the frame's code owner.
        let library_id = self
            .resolve_library_id_for_frame(&isolate_id, frame_index)
            .await?;

        // Step 3: fetch the full Library object.
        let library = with_timeout(
            self.backend
                .get_object(&isolate_id, &library_id, None, None),
        )
        .await
        .map_err(|e| format!("Failed to get library object '{}': {}", library_id, e))?;

        // Step 4: read library.variables — array of FieldRef.
        let field_refs: Vec<serde_json::Value> = library
            .get("variables")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Step 5: for each field, fetch its static value and convert to DapVariable.
        let mut result = Vec::with_capacity(field_refs.len().min(MAX_VARIABLES_PER_REQUEST));
        for field_ref in field_refs.iter().take(MAX_VARIABLES_PER_REQUEST) {
            let field_name = field_ref
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("?")
                .to_string();
            let field_id = match field_ref.get("id").and_then(|i| i.as_str()) {
                Some(id) => id.to_string(),
                None => {
                    // No field ID — emit a placeholder.
                    result.push(DapVariable {
                        name: field_name,
                        value: "<not initialized>".to_string(),
                        variables_reference: 0,
                        ..Default::default()
                    });
                    continue;
                }
            };

            // Fetch the full Field object to read staticValue.
            let field_obj =
                with_timeout(self.backend.get_object(&isolate_id, &field_id, None, None))
                    .await
                    .map_err(|e| format!("Failed to get field '{}': {}", field_id, e))?;

            // Determine const/static attributes.
            let is_const = field_obj
                .get("isConst")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let attributes: Vec<String> = if is_const {
                vec![
                    "static".to_string(),
                    "readOnly".to_string(),
                    "constant".to_string(),
                ]
            } else {
                vec!["static".to_string()]
            };

            let visibility = if field_name.starts_with('_') {
                Some("private".to_string())
            } else {
                None
            };

            let hint = crate::protocol::types::DapVariablePresentationHint {
                kind: None,
                attributes: Some(attributes),
                visibility,
                lazy: None,
            };

            // Read staticValue — may be absent (uninitialized sentinel).
            let static_value = field_obj.get("staticValue").cloned();

            let isolate_id_clone = isolate_id.clone();
            let var = match static_value {
                None => DapVariable {
                    name: field_name,
                    value: "<not initialized>".to_string(),
                    variables_reference: 0,
                    presentation_hint: Some(hint),
                    ..Default::default()
                },
                Some(sv) => {
                    // Check for the Sentinel type (uninitialized in VM Service).
                    let sv_type = sv.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if sv_type == "Sentinel" {
                        DapVariable {
                            name: field_name,
                            value: "<not initialized>".to_string(),
                            variables_reference: 0,
                            presentation_hint: Some(hint),
                            ..Default::default()
                        }
                    } else {
                        // Use the field name as evaluateName for globals —
                        // global statics are accessible by name directly.
                        let mut v = self.instance_ref_to_variable_with_eval_name(
                            &field_name,
                            &sv,
                            &isolate_id_clone,
                            Some(&field_name),
                        );
                        v.presentation_hint = Some(hint);
                        v
                    }
                }
            };
            result.push(var);
        }

        Ok(result)
    }

    /// Resolve the library ID for a given stack frame.
    ///
    /// Inspects `frame.code.owner`:
    /// - If owner `type` is `"Library"`, returns `owner.id` directly.
    /// - If owner `type` is `"Class"` (or `"ClassRef"`), follows `owner.library.id`.
    /// - Otherwise, falls back to `isolate.rootLib.id` via `get_isolate`.
    async fn resolve_library_id_for_frame(
        &self,
        isolate_id: &str,
        frame_index: i32,
    ) -> Result<String, String> {
        // Fetch the stack to examine code.owner for the target frame.
        let stack = with_timeout(self.backend.get_stack(isolate_id, Some(frame_index + 1)))
            .await
            .map_err(|e| format!("Failed to get stack: {}", e))?;

        let frames = stack
            .get("frames")
            .and_then(|f| f.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        if let Some(frame) = frames.get(frame_index as usize) {
            // Try code.owner to find the library.
            if let Some(owner) = frame.get("code").and_then(|c| c.get("owner")) {
                let owner_type = owner.get("type").and_then(|t| t.as_str()).unwrap_or("");

                // Direct library ref.
                if owner_type == "Library" || owner_type == "@Library" {
                    if let Some(lib_id) = owner.get("id").and_then(|i| i.as_str()) {
                        return Ok(lib_id.to_string());
                    }
                }

                // Class ref — the library is nested inside.
                if owner_type == "Class" || owner_type == "@Class" || owner_type == "ClassRef" {
                    if let Some(lib_id) = owner
                        .get("library")
                        .and_then(|l| l.get("id"))
                        .and_then(|i| i.as_str())
                    {
                        return Ok(lib_id.to_string());
                    }
                }
            }
        }

        // Fallback: use the isolate's rootLib.
        self.get_root_lib_from_isolate(isolate_id).await
    }

    /// Fetch the root library ID from the isolate object.
    ///
    /// Calls `get_isolate` and reads `isolate.rootLib.id`. This is the fallback
    /// used when the frame has no usable `code.owner`.
    async fn get_root_lib_from_isolate(&self, isolate_id: &str) -> Result<String, String> {
        let isolate = with_timeout(self.backend.get_isolate(isolate_id))
            .await
            .map_err(|e| format!("Failed to get isolate '{}': {}", isolate_id, e))?;

        isolate
            .get("rootLib")
            .and_then(|lib| lib.get("id"))
            .and_then(|id| id.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("Isolate '{}' has no rootLib", isolate_id))
    }

    /// Convert a VM Service `InstanceRef` JSON value to a DAP [`DapVariable`].
    ///
    /// Primitives (`Null`, `Bool`, `Int`, `Double`, `String`) are rendered
    /// inline with `variables_reference: 0` (no expansion). Complex types
    /// (collections and plain instances) are allocated a variable reference
    /// that the IDE can use to drill in further.
    ///
    /// This is the public 3-argument form that delegates to the internal
    /// implementation with `evaluate_name: None`. Use
    /// `instance_ref_to_variable_with_eval_name` internally when an
    /// `evaluateName` expression is available.
    pub(super) fn instance_ref_to_variable(
        &mut self,
        name: &str,
        instance_ref: &serde_json::Value,
        isolate_id: &str,
    ) -> DapVariable {
        self.instance_ref_to_variable_with_eval_name(name, instance_ref, isolate_id, None)
    }

    /// Internal implementation of `instance_ref_to_variable` that accepts an
    /// optional `evaluateName` expression.
    ///
    /// When `evaluate_name` is `Some`, the returned [`DapVariable`] will have
    /// its `evaluate_name` field set. For expandable types (collections and
    /// instances), the expression is also stored in `evaluate_name_map` keyed
    /// by the allocated variable reference so that `expand_object` can
    /// construct child expressions.
    fn instance_ref_to_variable_with_eval_name(
        &mut self,
        name: &str,
        instance_ref: &serde_json::Value,
        isolate_id: &str,
        evaluate_name: Option<&str>,
    ) -> DapVariable {
        let kind = instance_ref
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("");
        // Try "classRef" first (typed Stack serialization via serde camelCase rename),
        // then fall back to "class" (raw VM wire format from get_object/expand_object path).
        let class_name = instance_ref
            .get("classRef")
            .or_else(|| instance_ref.get("class"))
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
                evaluate_name: evaluate_name.map(|s| s.to_string()),
                ..Default::default()
            },

            "Bool" => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("false").to_string(),
                type_field: Some("bool".to_string()),
                variables_reference: 0,
                evaluate_name: evaluate_name.map(|s| s.to_string()),
                ..Default::default()
            },

            "Int" | "Double" => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("0").to_string(),
                type_field: Some(kind.to_lowercase()),
                variables_reference: 0,
                evaluate_name: evaluate_name.map(|s| s.to_string()),
                ..Default::default()
            },

            "String" => {
                let val = value_as_string.unwrap_or("");
                let truncated = instance_ref
                    .get("valueAsStringIsTruncated")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let display = if truncated {
                    format!("\"{}...\"", val)
                } else {
                    format!("\"{}\"", val)
                };
                // Allocate a variable reference for truncated strings so the
                // user can expand to inspect the full value via `expand_object`.
                let var_ref = if truncated {
                    if let Some(id) = obj_id {
                        let r = self.var_store.allocate(VariableRef::Object {
                            isolate_id: isolate_id.to_string(),
                            object_id: id.to_string(),
                        });
                        if let Some(en) = evaluate_name {
                            self.evaluate_name_map.insert(r, en.to_string());
                        }
                        r
                    } else {
                        0
                    }
                } else {
                    0
                };
                DapVariable {
                    name: name.to_string(),
                    value: display,
                    type_field: Some("String".to_string()),
                    variables_reference: var_ref,
                    evaluate_name: evaluate_name.map(|s| s.to_string()),
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
                    let r = self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    });
                    if let Some(en) = evaluate_name {
                        self.evaluate_name_map.insert(r, en.to_string());
                    }
                    r
                } else {
                    0
                };

                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some(type_name.to_string()),
                    variables_reference: var_ref,
                    indexed_variables: Some(length),
                    evaluate_name: evaluate_name.map(|s| s.to_string()),
                    ..Default::default()
                }
            }

            // ── Record types: expandable via fields ─────────────────────────
            "Record" => {
                let length = instance_ref
                    .get("length")
                    .and_then(|l| l.as_i64())
                    .unwrap_or(0);
                let display = format!("Record ({} fields)", length);
                let var_ref = if let Some(id) = obj_id {
                    let r = self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    });
                    if let Some(en) = evaluate_name {
                        self.evaluate_name_map.insert(r, en.to_string());
                    }
                    r
                } else {
                    0
                };
                DapVariable {
                    name: name.to_string(),
                    value: display,
                    type_field: Some("Record".to_string()),
                    variables_reference: var_ref,
                    evaluate_name: evaluate_name.map(|s| s.to_string()),
                    ..Default::default()
                }
            }

            // ── WeakReference: expandable to inspect the target ──────────────
            "WeakReference" => {
                let var_ref = if let Some(id) = obj_id {
                    let r = self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    });
                    if let Some(en) = evaluate_name {
                        self.evaluate_name_map.insert(r, en.to_string());
                    }
                    r
                } else {
                    0
                };
                DapVariable {
                    name: name.to_string(),
                    value: "WeakReference".to_string(),
                    type_field: Some("WeakReference".to_string()),
                    variables_reference: var_ref,
                    evaluate_name: evaluate_name.map(|s| s.to_string()),
                    ..Default::default()
                }
            }

            // ── Sentinel: optimized-out or otherwise inaccessible ────────────
            "Sentinel" => DapVariable {
                name: name.to_string(),
                value: value_as_string
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "<optimized out>".to_string()),
                type_field: Some("Sentinel".to_string()),
                variables_reference: 0,
                evaluate_name: evaluate_name.map(|s| s.to_string()),
                ..Default::default()
            },

            // ── Plain instances: expandable via fields ───────────────────────
            "PlainInstance" | "Closure" | "RegExp" | "Type" | "StackTrace" => {
                let type_name = class_name.unwrap_or(kind);
                let value = value_as_string
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{} instance", type_name));

                let var_ref = if let Some(id) = obj_id {
                    let r = self.var_store.allocate(VariableRef::Object {
                        isolate_id: isolate_id.to_string(),
                        object_id: id.to_string(),
                    });
                    if let Some(en) = evaluate_name {
                        self.evaluate_name_map.insert(r, en.to_string());
                    }
                    r
                } else {
                    0
                };

                DapVariable {
                    name: name.to_string(),
                    value,
                    type_field: Some(type_name.to_string()),
                    variables_reference: var_ref,
                    evaluate_name: evaluate_name.map(|s| s.to_string()),
                    ..Default::default()
                }
            }

            // ── Fallback ─────────────────────────────────────────────────────
            _ => DapVariable {
                name: name.to_string(),
                value: value_as_string.unwrap_or("<unknown>").to_string(),
                type_field: class_name.map(|s| s.to_string()),
                variables_reference: 0,
                evaluate_name: evaluate_name.map(|s| s.to_string()),
                ..Default::default()
            },
        }
    }

    /// Evaluate a lazy getter on demand (for `GetterEval` variable references).
    ///
    /// Called when the user explicitly expands a getter that was deferred
    /// because `evaluateGettersInDebugViews` was `false`. Returns a single
    /// `DapVariable` with the getter's evaluated value.
    ///
    /// A 1-second timeout is applied. On timeout, the variable value is
    /// `"<timed out>"`. On VM Service error, the value is `"<error: {message}>"`.
    async fn evaluate_lazy_getter(
        &mut self,
        isolate_id: &str,
        instance_id: &str,
        getter_name: &str,
    ) -> Result<Vec<DapVariable>, String> {
        let result = tokio::time::timeout(
            GETTER_EVAL_TIMEOUT,
            self.backend.evaluate(isolate_id, instance_id, getter_name),
        )
        .await;

        let var = match result {
            Err(_timeout) => DapVariable {
                name: getter_name.to_string(),
                value: "<timed out>".to_string(),
                variables_reference: 0,
                presentation_hint: Some(DapVariablePresentationHint {
                    attributes: Some(vec!["hasSideEffects".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Ok(Err(e)) => DapVariable {
                name: getter_name.to_string(),
                value: format!("<error: {}>", e),
                variables_reference: 0,
                presentation_hint: Some(DapVariablePresentationHint {
                    attributes: Some(vec!["hasSideEffects".to_string()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            Ok(Ok(instance_ref)) => {
                let mut var = self.instance_ref_to_variable(getter_name, &instance_ref, isolate_id);
                var.presentation_hint = Some(DapVariablePresentationHint {
                    attributes: Some(vec!["hasSideEffects".to_string()]),
                    ..Default::default()
                });
                var
            }
        };

        Ok(vec![var])
    }

    /// Collect getter method names from a class and its superclass hierarchy.
    ///
    /// Traverses the class chain starting from `class_id` up to
    /// [`MAX_SUPERCLASS_DEPTH`] levels, calling `get_object` for each class
    /// and filtering its `functions` array for `ImplicitGetter` or `Getter`
    /// kinds that are not static and not in [`FILTERED_GETTER_NAMES`].
    ///
    /// Returns at most [`MAX_GETTER_EVALUATIONS`] getter names to prevent
    /// hanging when an object has an unusually large class hierarchy.
    async fn collect_getters_from_class(&self, isolate_id: &str, class_id: &str) -> Vec<String> {
        let mut getters: Vec<String> = Vec::new();
        let mut current_class_id = class_id.to_string();

        for _depth in 0..MAX_SUPERCLASS_DEPTH {
            if getters.len() >= MAX_GETTER_EVALUATIONS {
                break;
            }

            let class_obj = match with_timeout(self.backend.get_object(
                isolate_id,
                &current_class_id,
                None,
                None,
            ))
            .await
            {
                Ok(obj) => obj,
                Err(_) => break,
            };

            // Read functions array from the class object.
            let functions: &[serde_json::Value] = class_obj
                .get("functions")
                .and_then(|f| f.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[]);

            for func in functions {
                if getters.len() >= MAX_GETTER_EVALUATIONS {
                    break;
                }

                let kind = func.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                // Accept ImplicitGetter (auto-generated field getter) and
                // explicit Getter functions.
                if kind != "ImplicitGetter" && kind != "Getter" {
                    continue;
                }

                // Skip static getters — they are not accessible on the instance.
                let is_static = func
                    .get("static")
                    .and_then(|s| s.as_bool())
                    .unwrap_or(false);
                if is_static {
                    continue;
                }

                let name = match func.get("name").and_then(|n| n.as_str()) {
                    Some(n) => n,
                    None => continue,
                };

                // Filter out internal getters.
                if FILTERED_GETTER_NAMES.contains(&name) {
                    continue;
                }

                // Avoid duplicates (same getter may appear from multiple
                // traversal paths if the hierarchy is unusual).
                if !getters.iter().any(|g| g == name) {
                    getters.push(name.to_string());
                }
            }

            // Traverse to superclass. Stop if super is absent or is "Object"
            // (the root of all Dart class hierarchies — its getters are
            // already covered by FILTERED_GETTER_NAMES).
            let super_class = match class_obj.get("super") {
                Some(s) => s,
                None => break,
            };

            // If the super is JSON null, we've reached the root.
            if super_class.is_null() {
                break;
            }

            let super_name = super_class
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            if super_name == "Object" {
                break;
            }

            let super_id = match super_class.get("id").and_then(|i| i.as_str()) {
                Some(id) => id.to_string(),
                None => break,
            };

            current_class_id = super_id;
        }

        getters
    }

    /// Expand a VM Service object into a list of [`DapVariable`] children.
    ///
    /// Fetches the full object via `get_object` and dispatches based on the
    /// object's `kind`:
    ///
    /// - `List` / typed arrays — indexed elements `[0]`, `[1]`, …
    /// - `Map` — keyed entries `[key]`, …
    /// - `PlainInstance` and others — named fields plus evaluated getters
    ///
    /// For `PlainInstance` objects, getter methods from the class hierarchy are
    /// also collected and either eagerly evaluated (when
    /// `evaluate_getters_in_debug_views` is `true`) or shown as lazy items.
    ///
    /// The `start` and `count` paging parameters are forwarded to the VM
    /// Service so that large collections can be fetched in chunks.
    ///
    /// `parent_evaluate_name` is the `evaluateName` expression for the parent
    /// object being expanded (e.g., `"myList"`, `"obj"`). When provided, child
    /// variables are assigned `evaluateName` expressions of the form:
    /// - Fields: `parent.fieldName`
    /// - Indexed elements: `parent[index]`
    /// - Map string keys: `parent["key"]`
    /// - Map integer keys: `parent[42]`
    async fn expand_object(
        &mut self,
        isolate_id: &str,
        object_id: &str,
        start: Option<i64>,
        count: Option<i64>,
        parent_evaluate_name: Option<&str>,
    ) -> Result<Vec<DapVariable>, String> {
        let obj =
            with_timeout(self.backend.get_object(isolate_id, object_id, start, count)).await?;

        let obj_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match obj_type {
            "Instance" => {
                let kind = obj.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                match kind {
                    // Sets are stored in the VM Service like Lists — they have
                    // an `elements` array. Include "Set" here so it uses the
                    // correct indexed-expansion path instead of falling through
                    // to the fields path (which returns nothing for Sets).
                    "List" | "Set" | "Uint8List" | "Uint8ClampedList" | "Int32List"
                    | "Float64List" => {
                        // Expand list/set elements.
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
                            // Construct child evaluateName: parent[index]
                            let child_eval_name: Option<String> =
                                parent_evaluate_name.map(|p| format!("{}[{}]", p, index));
                            result.push(self.instance_ref_to_variable_with_eval_name(
                                &elem_name,
                                elem,
                                &isolate_id,
                                child_eval_name.as_deref(),
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
                            let key_val = assoc.get("key");
                            let key_str = key_val
                                .and_then(|k| k.get("valueAsString"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            // Determine the key kind to choose the right
                            // evaluateName format:
                            // - String keys: parent["key"]
                            // - Int keys:    parent[42]
                            // - Other:       parent[key] (same as display)
                            let key_kind = key_val
                                .and_then(|k| k.get("kind"))
                                .and_then(|k| k.as_str())
                                .unwrap_or("");
                            let value = assoc
                                .get("value")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let entry_name = format!("[{}]", key_str);
                            // Construct child evaluateName based on key kind.
                            let child_eval_name: Option<String> =
                                parent_evaluate_name.map(|p| match key_kind {
                                    "String" => {
                                        format!("{}[\"{}\"]", p, escape_dart_string(key_str))
                                    }
                                    "Int" => format!("{}[{}]", p, key_str),
                                    _ => format!("{}[{}]", p, key_str),
                                });
                            result.push(self.instance_ref_to_variable_with_eval_name(
                                &entry_name,
                                &value,
                                &isolate_id,
                                child_eval_name.as_deref(),
                            ));
                        }
                        Ok(result)
                    }

                    // Records use the same `fields` structure as PlainInstance.
                    // Positional fields have names like "$1", "$2"; named
                    // fields use their actual name.
                    "Record" => {
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
                            // Construct child evaluateName: parent.fieldName
                            let child_eval_name: Option<String> =
                                parent_evaluate_name.map(|p| format!("{}.{}", p, name));
                            result.push(self.instance_ref_to_variable_with_eval_name(
                                &name,
                                &value,
                                &isolate_id,
                                child_eval_name.as_deref(),
                            ));
                        }
                        Ok(result)
                    }

                    // WeakReference has a `target` field that may be JSON null
                    // (absent key) or a VM Service Null instance if the target
                    // was garbage collected.
                    "WeakReference" => {
                        let raw_target = obj.get("target").cloned();
                        let isolate_id = isolate_id.to_string();
                        // Normalise: if the target is absent or JSON null, treat
                        // it as the VM Service Null kind so instance_ref_to_variable
                        // renders it as "null".
                        let target = match raw_target {
                            Some(t) if !t.is_null() => t,
                            _ => serde_json::json!({ "kind": "Null" }),
                        };
                        // WeakReference.target is accessed via `.target` field.
                        let child_eval_name: Option<String> =
                            parent_evaluate_name.map(|p| format!("{}.target", p));
                        let var = self.instance_ref_to_variable_with_eval_name(
                            "target",
                            &target,
                            &isolate_id,
                            child_eval_name.as_deref(),
                        );
                        Ok(vec![var])
                    }

                    // PlainInstance: expand fields and optionally evaluate
                    // getters from the class hierarchy.
                    "PlainInstance" => {
                        let fields: Vec<serde_json::Value> = obj
                            .get("fields")
                            .and_then(|f| f.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let isolate_id = isolate_id.to_string();
                        let obj_id = object_id.to_string();

                        let mut result = Vec::with_capacity(fields.len());
                        for field in &fields {
                            // Skip TypeArguments entries — they are internal VM
                            // details not meaningful to the user.
                            let field_type =
                                field.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if field_type == "@TypeArguments" || field_type == "TypeArguments" {
                                continue;
                            }

                            let name = field
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("?")
                                .to_string();
                            let value = field
                                .get("value")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            // Construct child evaluateName: parent.fieldName
                            let child_eval_name: Option<String> =
                                parent_evaluate_name.map(|p| format!("{}.{}", p, name));
                            result.push(self.instance_ref_to_variable_with_eval_name(
                                &name,
                                &value,
                                &isolate_id,
                                child_eval_name.as_deref(),
                            ));
                        }

                        // Collect getters from the class hierarchy and append them.
                        // Read the class ID from the instance's "class" field.
                        let class_id = obj
                            .get("class")
                            .and_then(|c| c.get("id"))
                            .and_then(|i| i.as_str())
                            .map(|s| s.to_string());

                        if let Some(class_id) = class_id {
                            let getter_names = self
                                .collect_getters_from_class(&isolate_id, &class_id)
                                .await;
                            let evaluate_getters = self.evaluate_getters_in_debug_views;

                            // Set a total budget for eager getter evaluation so that
                            // objects with many getters don't block the IDE panel.
                            // Only applies when evaluate_getters is true; lazy mode
                            // has no network calls so no budget is needed.
                            let getter_deadline = if evaluate_getters {
                                Some(tokio::time::Instant::now() + GETTER_EVAL_TOTAL_BUDGET)
                            } else {
                                None
                            };

                            for (getter_idx, getter_name) in getter_names.iter().enumerate() {
                                if evaluate_getters {
                                    // Check the total budget before each call.
                                    if let Some(deadline) = getter_deadline {
                                        if tokio::time::Instant::now() >= deadline {
                                            tracing::debug!(
                                                "Getter evaluation budget exhausted ({:?}), showing remaining {} getters as lazy",
                                                GETTER_EVAL_TOTAL_BUDGET,
                                                getter_names.len() - getter_idx,
                                            );
                                            // Add remaining getters as lazy items so the
                                            // user can still expand them on demand.
                                            for remaining_name in &getter_names[getter_idx..] {
                                                let getter_ref = self.var_store.allocate(
                                                    VariableRef::GetterEval {
                                                        isolate_id: isolate_id.clone(),
                                                        instance_id: obj_id.clone(),
                                                        getter_name: remaining_name.clone(),
                                                    },
                                                );
                                                result.push(DapVariable {
                                                    name: remaining_name.clone(),
                                                    value: String::new(),
                                                    variables_reference: getter_ref,
                                                    presentation_hint: Some(
                                                        DapVariablePresentationHint {
                                                            lazy: Some(true),
                                                            ..Default::default()
                                                        },
                                                    ),
                                                    ..Default::default()
                                                });
                                            }
                                            break;
                                        }
                                    }

                                    // Eagerly evaluate the getter with a 1-second timeout.
                                    let eval_result = tokio::time::timeout(
                                        GETTER_EVAL_TIMEOUT,
                                        self.backend.evaluate(&isolate_id, &obj_id, getter_name),
                                    )
                                    .await;

                                    let var = match eval_result {
                                        Err(_timeout) => DapVariable {
                                            name: getter_name.clone(),
                                            value: "<timed out>".to_string(),
                                            variables_reference: 0,
                                            presentation_hint: Some(DapVariablePresentationHint {
                                                attributes: Some(
                                                    vec!["hasSideEffects".to_string()],
                                                ),
                                                ..Default::default()
                                            }),
                                            ..Default::default()
                                        },
                                        Ok(Err(e)) => DapVariable {
                                            name: getter_name.clone(),
                                            value: format!("<error: {}>", e),
                                            variables_reference: 0,
                                            presentation_hint: Some(DapVariablePresentationHint {
                                                attributes: Some(
                                                    vec!["hasSideEffects".to_string()],
                                                ),
                                                ..Default::default()
                                            }),
                                            ..Default::default()
                                        },
                                        Ok(Ok(instance_ref)) => {
                                            let mut var = self.instance_ref_to_variable(
                                                getter_name,
                                                &instance_ref,
                                                &isolate_id,
                                            );
                                            var.presentation_hint =
                                                Some(DapVariablePresentationHint {
                                                    attributes: Some(vec![
                                                        "hasSideEffects".to_string()
                                                    ]),
                                                    ..Default::default()
                                                });
                                            var
                                        }
                                    };
                                    result.push(var);
                                } else {
                                    // Lazy getter: show a placeholder that the user can expand.
                                    let getter_ref =
                                        self.var_store.allocate(VariableRef::GetterEval {
                                            isolate_id: isolate_id.clone(),
                                            instance_id: obj_id.clone(),
                                            getter_name: getter_name.clone(),
                                        });
                                    result.push(DapVariable {
                                        name: getter_name.clone(),
                                        value: String::new(),
                                        variables_reference: getter_ref,
                                        presentation_hint: Some(DapVariablePresentationHint {
                                            lazy: Some(true),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    });
                                }
                            }
                        }

                        Ok(result)
                    }

                    _ => {
                        // Expand instance fields (Closure, RegExp, Type,
                        // StackTrace, and any other instance kind).
                        let fields: Vec<serde_json::Value> = obj
                            .get("fields")
                            .and_then(|f| f.as_array())
                            .cloned()
                            .unwrap_or_default();
                        let isolate_id = isolate_id.to_string();

                        let mut result = Vec::with_capacity(fields.len());
                        for field in &fields {
                            // Skip TypeArguments entries — they are internal VM
                            // details not meaningful to the user.
                            let field_type =
                                field.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            if field_type == "@TypeArguments" || field_type == "TypeArguments" {
                                continue;
                            }

                            let name = field
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("?")
                                .to_string();
                            let value = field
                                .get("value")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            // Construct child evaluateName: parent.fieldName
                            let child_eval_name: Option<String> =
                                parent_evaluate_name.map(|p| format!("{}.{}", p, name));
                            result.push(self.instance_ref_to_variable_with_eval_name(
                                &name,
                                &value,
                                &isolate_id,
                                child_eval_name.as_deref(),
                            ));
                        }
                        Ok(result)
                    }
                }
            }
            _ => Ok(Vec::new()),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// toString() enrichment helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Escape a string for use inside a Dart double-quoted string literal.
///
/// Handles `"`, `\`, `$`, `\n`, `\r`, and `\t` so that the resulting
/// expression (e.g., `myMap["hello \"world\""]`) is valid Dart.
fn escape_dart_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '$' => escaped.push_str("\\$"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Build a [`ToStringCandidate`] for an `InstanceRef` JSON value if it is a
/// kind that benefits from `toString()` enrichment.
///
/// Returns `None` for primitives, collections, sentinels, and other kinds
/// that already have useful display values without calling `toString()`.
fn to_string_candidate(
    var_index: usize,
    isolate_id: &str,
    instance_ref: &serde_json::Value,
) -> Option<ToStringCandidate> {
    let kind = instance_ref.get("kind").and_then(|k| k.as_str())?;
    if !TO_STRING_KINDS.contains(&kind) {
        return None;
    }
    let object_id = instance_ref.get("id").and_then(|i| i.as_str())?;
    // Derive the class name the same way instance_ref_to_variable_with_eval_name does.
    let class_name = instance_ref
        .get("classRef")
        .or_else(|| instance_ref.get("class"))
        .and_then(|c| c.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or(kind)
        .to_string();
    Some(ToStringCandidate {
        var_index,
        isolate_id: isolate_id.to_string(),
        object_id: object_id.to_string(),
        class_name,
    })
}

#[cfg(test)]
mod escape_tests {
    use super::escape_dart_string;

    #[test]
    fn test_escape_dart_string_quotes() {
        assert_eq!(escape_dart_string(r#"hello "world""#), r#"hello \"world\""#);
    }

    #[test]
    fn test_escape_dart_string_backslash() {
        assert_eq!(escape_dart_string(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn test_escape_dart_string_dollar() {
        assert_eq!(escape_dart_string("cost: $100"), r"cost: \$100");
    }

    #[test]
    fn test_escape_dart_string_newline() {
        assert_eq!(escape_dart_string("line1\nline2"), r"line1\nline2");
    }

    #[test]
    fn test_escape_dart_string_carriage_return() {
        assert_eq!(escape_dart_string("line1\rline2"), r"line1\rline2");
    }

    #[test]
    fn test_escape_dart_string_tab() {
        assert_eq!(escape_dart_string("col1\tcol2"), r"col1\tcol2");
    }

    #[test]
    fn test_escape_dart_string_no_special_chars() {
        assert_eq!(escape_dart_string("hello world"), "hello world");
    }

    #[test]
    fn test_escape_dart_string_empty() {
        assert_eq!(escape_dart_string(""), "");
    }

    #[test]
    fn test_escape_dart_string_multiple_escapes() {
        assert_eq!(
            escape_dart_string("say \"hello\" for $5\nbye"),
            r#"say \"hello\" for \$5\nbye"#
        );
    }
}
