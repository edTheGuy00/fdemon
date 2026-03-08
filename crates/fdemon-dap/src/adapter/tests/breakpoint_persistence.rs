//! Tests for breakpoint persistence across hot restart (IsolateExit / IsolateRunnable cycle).

use super::{
    drain_breakpoint_events, make_set_breakpoints_request, make_set_exception_breakpoints_request,
};
use crate::adapter::test_helpers::*;
use crate::adapter::*;

#[tokio::test]
async fn test_desired_breakpoints_survive_isolate_exit() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    // Register isolate and set breakpoints.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[25, 30]);
    adapter.handle_request(&req).await;

    // Simulate hot restart: isolate exits.
    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/1".into(),
        })
        .await;

    // Active breakpoints must be cleared.
    assert!(
        adapter.breakpoint_state.is_empty(),
        "Active breakpoints must be cleared on IsolateExit"
    );

    // Desired breakpoints must survive.
    let uri = "file:///lib/main.dart";
    let desired = adapter.desired_breakpoints.get(uri);
    assert!(
        desired.is_some(),
        "Desired breakpoints must survive IsolateExit"
    );
    let desired = desired.unwrap();
    assert_eq!(desired.len(), 2, "Both desired breakpoints must survive");
}

#[tokio::test]
async fn test_isolate_exit_emits_unverified_breakpoint_events() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    // Register isolate and set breakpoints.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[25, 30]);
    adapter.handle_request(&req).await;

    // Drain events from setBreakpoints.
    while rx.try_recv().is_ok() {}

    // Simulate isolate exit.
    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/1".into(),
        })
        .await;

    // Should get unverified breakpoint events plus thread exited event.
    let bp_events = drain_breakpoint_events(&mut rx).await;
    assert_eq!(
        bp_events.len(),
        2,
        "Should emit 2 unverified breakpoint events on IsolateExit, got: {:?}",
        bp_events
    );
    for ev in &bp_events {
        assert_eq!(
            ev["breakpoint"]["verified"], false,
            "Breakpoints must be unverified on exit: {:?}",
            ev
        );
    }
}

#[tokio::test]
async fn test_isolate_runnable_reapplies_breakpoints_to_new_isolate() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    // Set up initial isolate and breakpoints.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[25, 30]);
    let resp = adapter.handle_request(&req).await;
    assert!(resp.success);

    // Capture DAP IDs from the first set.
    let body = resp.body.unwrap();
    let bps = body["breakpoints"].as_array().unwrap();
    let id1 = bps[0]["id"].as_i64().unwrap();
    let id2 = bps[1]["id"].as_i64().unwrap();

    // Drain all pending events.
    while rx.try_recv().is_ok() {}

    // Hot restart: old isolate exits.
    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/1".into(),
        })
        .await;
    while rx.try_recv().is_ok() {}

    // New isolate starts.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/2".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok(); // thread started

    // New isolate becomes runnable → breakpoints are re-applied.
    adapter
        .handle_debug_event(DebugEvent::IsolateRunnable {
            isolate_id: "isolates/2".into(),
        })
        .await;

    // Active breakpoints must be re-populated.
    assert_eq!(
        adapter.breakpoint_state.len(),
        2,
        "Both breakpoints must be re-applied on IsolateRunnable"
    );

    // Breakpoint events with verified: true must be emitted.
    let bp_events = drain_breakpoint_events(&mut rx).await;
    assert_eq!(
        bp_events.len(),
        2,
        "Should emit 2 verified breakpoint events on IsolateRunnable"
    );
    for ev in &bp_events {
        assert_eq!(
            ev["breakpoint"]["verified"], true,
            "Re-applied breakpoints must be verified: {:?}",
            ev
        );
    }

    // The stable DAP IDs must be preserved.
    let reapplied_ids: Vec<i64> = bp_events
        .iter()
        .filter_map(|ev| ev["breakpoint"]["id"].as_i64())
        .collect();
    assert!(
        reapplied_ids.contains(&id1),
        "DAP ID {} must be preserved across restart",
        id1
    );
    assert!(
        reapplied_ids.contains(&id2),
        "DAP ID {} must be preserved across restart",
        id2
    );
}

#[tokio::test]
async fn test_no_duplicate_breakpoints_after_multiple_restarts() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    // Set up initial breakpoints.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[25]);
    adapter.handle_request(&req).await;
    while rx.try_recv().is_ok() {}

    // Simulate 3 restart cycles.
    for cycle in 2..=4usize {
        let old_id = format!("isolates/{}", cycle - 1);
        let new_id = format!("isolates/{}", cycle);

        adapter
            .handle_debug_event(DebugEvent::IsolateExit { isolate_id: old_id })
            .await;
        while rx.try_recv().is_ok() {}

        adapter
            .handle_debug_event(DebugEvent::IsolateStart {
                isolate_id: new_id.clone(),
                name: "main".into(),
            })
            .await;
        rx.try_recv().ok();

        adapter
            .handle_debug_event(DebugEvent::IsolateRunnable { isolate_id: new_id })
            .await;
        while rx.try_recv().is_ok() {}

        assert_eq!(
            adapter.breakpoint_state.len(),
            1,
            "Exactly 1 active breakpoint after restart cycle {}, not duplicates",
            cycle
        );
    }
}

#[tokio::test]
async fn test_exception_mode_reapplied_on_isolate_runnable() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    // Set up isolate and configure exception mode.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // Set exception pause mode to "All".
    let req = make_set_exception_breakpoints_request(1, &["All"]);
    adapter.handle_request(&req).await;
    assert_eq!(adapter.exception_mode, DapExceptionPauseMode::All);

    // Simulate restart.
    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/1".into(),
        })
        .await;
    while rx.try_recv().is_ok() {}

    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/2".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    // IsolateRunnable must re-apply the exception mode.
    // MockBackend silently succeeds, so we verify no panic and mode is still set.
    adapter
        .handle_debug_event(DebugEvent::IsolateRunnable {
            isolate_id: "isolates/2".into(),
        })
        .await;

    // Exception mode must still be set in the adapter.
    assert_eq!(
        adapter.exception_mode,
        DapExceptionPauseMode::All,
        "Exception mode must survive hot restart"
    );
}

#[tokio::test]
async fn test_breakpoint_verification_sequence_during_restart() {
    // Verify the full sequence: verified → unverified → verified.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[10]);
    let resp = adapter.handle_request(&req).await;
    let body = resp.body.unwrap();
    let bps = body["breakpoints"].as_array().unwrap();
    // Phase 1: breakpoint is verified after initial set.
    assert_eq!(
        bps[0]["verified"], true,
        "Initial breakpoint must be verified"
    );
    while rx.try_recv().is_ok() {}

    // Phase 2: isolate exits → breakpoint becomes unverified.
    adapter
        .handle_debug_event(DebugEvent::IsolateExit {
            isolate_id: "isolates/1".into(),
        })
        .await;
    let bp_events = drain_breakpoint_events(&mut rx).await;
    assert!(
        !bp_events.is_empty(),
        "Unverified event must be emitted on exit"
    );
    assert_eq!(
        bp_events[0]["breakpoint"]["verified"], false,
        "Breakpoint must become unverified on isolate exit"
    );
    while rx.try_recv().is_ok() {}

    // Phase 3: new isolate runnable → breakpoint verified again.
    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/2".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::IsolateRunnable {
            isolate_id: "isolates/2".into(),
        })
        .await;

    let bp_events = drain_breakpoint_events(&mut rx).await;
    assert!(
        !bp_events.is_empty(),
        "Verified event must be emitted on runnable"
    );
    assert_eq!(
        bp_events[0]["breakpoint"]["verified"], true,
        "Breakpoint must be verified after re-application"
    );
}

#[tokio::test]
async fn test_isolate_runnable_with_no_desired_breakpoints_does_nothing() {
    // Edge case: IsolateRunnable fires when there are no desired breakpoints.
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    adapter
        .handle_debug_event(DebugEvent::IsolateRunnable {
            isolate_id: "isolates/1".into(),
        })
        .await;

    // No breakpoint events and no active breakpoints.
    let bp_events = drain_breakpoint_events(&mut rx).await;
    assert!(
        bp_events.is_empty(),
        "No breakpoint events expected when no desired breakpoints"
    );
    assert!(adapter.breakpoint_state.is_empty());
}

#[tokio::test]
async fn test_on_hot_restart_clears_active_breakpoints_not_desired() {
    let (mut adapter, mut rx) = DapAdapter::new(MockBackend);

    adapter
        .handle_debug_event(DebugEvent::IsolateStart {
            isolate_id: "isolates/1".into(),
            name: "main".into(),
        })
        .await;
    rx.try_recv().ok();

    let req = make_set_breakpoints_request(1, "/lib/main.dart", &[10, 20]);
    adapter.handle_request(&req).await;
    assert_eq!(adapter.breakpoint_state.len(), 2);

    // Trigger on_hot_restart.
    adapter.on_hot_restart();

    // Active breakpoints must be cleared.
    assert!(
        adapter.breakpoint_state.is_empty(),
        "on_hot_restart must clear active breakpoints"
    );

    // Desired breakpoints must still be intact.
    let uri = "file:///lib/main.dart";
    let desired = adapter.desired_breakpoints.get(uri);
    assert!(
        desired.map(|v| v.len()).unwrap_or(0) >= 1,
        "Desired breakpoints must survive on_hot_restart"
    );
}
