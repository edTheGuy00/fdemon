//! Integration tests for the DAP adapter.
//!
//! Each submodule covers a themed area of adapter functionality.
//! Shared helper functions are defined here as `pub(super)` so all
//! submodules can import them.

mod adapter_core;
mod attach_threads;
mod backend_phase6;
mod breakpoint_persistence;
mod breakpoints;
mod call_service;
mod conditional_breakpoints;
mod custom_events;
mod evaluate_name;
mod events_logging;
mod exception_info;
mod exception_scope;
mod execution;
mod getter_evaluation;
mod hot_operations;
mod loaded_sources;
mod logpoints;
mod production_hardening;
mod restart_frame;
mod stack_scopes_variables;
mod to_string_display;
mod variable_type_rendering;

// ─────────────────────────────────────────────────────────────────────────────
// Shared test helpers
// ─────────────────────────────────────────────────────────────────────────────

use crate::adapter::*;
use crate::DapRequest;

/// Build a minimal DAP request with no arguments.
pub(super) fn make_request(seq: i64, command: &str) -> DapRequest {
    DapRequest {
        seq,
        command: command.into(),
        arguments: None,
    }
}

/// Build a `setBreakpoints` request with a list of lines.
pub(super) fn make_set_breakpoints_request(
    seq: i64,
    source_path: &str,
    lines: &[i64],
) -> DapRequest {
    use crate::protocol::types::{DapSource, SourceBreakpoint};
    let breakpoints: Vec<SourceBreakpoint> = lines
        .iter()
        .map(|&l| SourceBreakpoint {
            line: l,
            ..Default::default()
        })
        .collect();
    DapRequest {
        seq,
        command: "setBreakpoints".into(),
        arguments: Some(serde_json::json!({
            "source": DapSource {
                path: Some(source_path.to_string()),
                ..Default::default()
            },
            "breakpoints": breakpoints,
        })),
    }
}

/// Build a `setExceptionBreakpoints` request.
pub(super) fn make_set_exception_breakpoints_request(seq: i64, filters: &[&str]) -> DapRequest {
    DapRequest {
        seq,
        command: "setExceptionBreakpoints".into(),
        arguments: Some(serde_json::json!({
            "filters": filters,
        })),
    }
}

/// Register an isolate on the adapter and return its thread ID.
pub(super) async fn register_isolate(
    adapter: &mut DapAdapter<impl DebugBackend>,
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
    isolate_id: &str,
) -> i64 {
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: isolate_id.into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();
    adapter
        .thread_map
        .thread_id_for(isolate_id)
        .expect("isolate should be registered")
}

/// Collect all DAP events from the channel without blocking.
pub(super) fn drain_events(
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
) -> Vec<crate::DapMessage> {
    let mut events = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        events.push(msg);
    }
    events
}

/// Drain all pending messages and collect `breakpoint` events.
pub(super) async fn drain_breakpoint_events(
    rx: &mut tokio::sync::mpsc::Receiver<crate::DapMessage>,
) -> Vec<serde_json::Value> {
    let mut events = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let crate::DapMessage::Event(e) = msg {
            if e.event == "breakpoint" {
                if let Some(body) = e.body {
                    events.push(body);
                }
            }
        }
    }
    events
}
