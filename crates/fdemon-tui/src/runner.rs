//! Main TUI runner - entry points and event loop
//!
//! Contains the core application lifecycle:
//! - `run_with_project`: Main entry point with Flutter project
//! - `run_with_project_and_dap`: Like `run_with_project` but with DAP port override and auto-start
//! - `run`: Demo/test entry point without Flutter
//! - `run_loop`: Main event loop processing terminal and daemon events

use std::path::Path;

use tracing::{error, warn};

use fdemon_app::config::should_auto_start_dap;
use fdemon_app::message::Message;
use fdemon_app::services::{Clipboard, NullClipboard, SystemClipboard};
use fdemon_app::spawn;
use fdemon_app::{Engine, ToastLevel, UpdateAction};
use fdemon_core::prelude::*;

use crate::{event, render, startup, terminal};

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Create the engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Initialize clipboard. Failures fall back to NullClipboard so that every
    // clipboard write returns an error, firing the runner's failure-toast path
    // and showing the user that copy is non-functional. The warn! log gives
    // operators a breadcrumb when this fallback is active.
    let mut clipboard: Box<dyn Clipboard> = match SystemClipboard::new() {
        Ok(cb) => Box::new(cb),
        Err(e) => {
            warn!("system clipboard unavailable: {e}");
            Box::new(NullClipboard)
        }
    };

    // Initialize terminal (TUI-specific)
    let mut term = ratatui::init();

    // Install panic hook AFTER ratatui::init() so fdemon's hook wraps
    // ratatui's. Both use the "take + wrap" set_hook pattern; whichever
    // installs last wraps the other. Hooks fire LIFO on panic, so this
    // order guarantees: disable_mouse_capture → ratatui::restore.
    terminal::install_panic_hook();

    // Enable mouse capture if the user has it on (default true). Failures are
    // logged and ignored so the rest of the TUI still works.
    if engine.settings.ui.enable_mouse {
        if let Err(e) = terminal::enable_mouse_capture() {
            warn!("mouse capture disabled: {e}");
        }
    }

    // TUI-specific startup: detect auto-start or show NewSessionDialog
    let startup_result =
        startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

    // Render first frame
    if let Err(e) = term.draw(|frame| render::view(frame, &mut engine.state)) {
        error!("Failed to render initial frame: {}", e);
    }

    // Trigger startup discovery (non-blocking)
    spawn::spawn_tool_availability_check(engine.msg_sender());

    // Dispatch based on auto-start detection
    dispatch_startup_action(&mut engine, startup_result);

    // Run the main loop
    let result = run_loop(&mut term, &mut engine, &mut *clipboard);

    // Disable mouse capture FIRST: stops the terminal from generating new
    // SGR mouse reports before we drain the TTY queue.
    terminal::disable_mouse_capture();

    // Drain any mouse SGR reports that were already buffered in the kernel
    // TTY queue before DisableMouseCapture took effect. Without this drain,
    // those bytes remain in the queue and the shell reads them after exit,
    // printing garbage on the command line.
    event::drain_input(std::time::Duration::from_millis(50));

    // Shutdown engine (stops watcher, cleans up sessions). Safe to call now —
    // no new SGR sequences will queue because capture is already disabled.
    engine.shutdown().await;

    // Restore terminal (TUI-specific)
    ratatui::restore();

    result
}

/// Run the TUI application with a Flutter project and optional DAP configuration.
///
/// This is identical to [`run_with_project`] but also:
/// 1. Applies a `--dap-port` CLI override to `settings.dap.port` and forces
///    `settings.dap.enabled = true` when `dap_port` is `Some(port)`.
/// 2. Applies a `--dap-config` IDE override to `AppState.cli_dap_config_override`
///    when `dap_config` is `Some(ide)`, bypassing environment-based detection.
/// 3. Evaluates [`should_auto_start_dap`] after CLI flag processing and sends
///    `Message::StartDapServer` if the result is `true`.
///
/// This covers all startup paths:
/// - `--dap-port` CLI flag → `dap.enabled = true` → auto-starts
/// - `dap.enabled = true` in config → auto-starts
/// - `dap.auto_start_in_ide = true` + IDE detected → auto-starts
/// - No DAP config + no IDE → does not auto-start
pub async fn run_with_project_and_dap(
    project_path: &Path,
    dap_port: Option<u16>,
    dap_config: Option<fdemon_app::config::ParentIde>,
) -> Result<()> {
    // Create the engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Apply --dap-port CLI override: sets port and forces enabled = true in
    // both settings copies, keeping them in sync.
    if let Some(port) = dap_port {
        engine.apply_cli_dap_override(port);
    }

    // Apply --dap-config IDE override: stored on AppState so handle_started()
    // can pass it to GenerateIdeConfig, bypassing environment-based detection.
    if let Some(ide) = dap_config {
        engine.apply_cli_dap_config_override(ide);
    }

    // Evaluate DAP auto-start (covers config-enabled and IDE-detected scenarios).
    // --dap-port already sets dap.enabled=true above, so this handles all paths.
    //
    // ORDERING: process_message is called synchronously before run_loop starts.
    // This is safe because StartDapServer returns an UpdateAction (async side
    // effect), not a follow-up Message. If the handler is changed to return a
    // follow-up Message, this call site must switch to
    // engine.msg_sender().try_send() to preserve ordering.
    if should_auto_start_dap(&engine.settings) {
        engine.process_message(Message::StartDapServer);
    }

    // Initialize clipboard (same fallback policy as run_with_project).
    let mut clipboard: Box<dyn Clipboard> = match SystemClipboard::new() {
        Ok(cb) => Box::new(cb),
        Err(e) => {
            warn!("system clipboard unavailable: {e}");
            Box::new(NullClipboard)
        }
    };

    // Initialize terminal (TUI-specific)
    let mut term = ratatui::init();

    // Install panic hook AFTER ratatui::init() so fdemon's hook wraps
    // ratatui's. Both use the "take + wrap" set_hook pattern; whichever
    // installs last wraps the other. Hooks fire LIFO on panic, so this
    // order guarantees: disable_mouse_capture → ratatui::restore.
    terminal::install_panic_hook();

    // Enable mouse capture if the user has it on (default true). Failures are
    // logged and ignored so the rest of the TUI still works.
    if engine.settings.ui.enable_mouse {
        if let Err(e) = terminal::enable_mouse_capture() {
            warn!("mouse capture disabled: {e}");
        }
    }

    // TUI-specific startup: detect auto-start or show NewSessionDialog
    let startup_result =
        startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

    // Render first frame
    if let Err(e) = term.draw(|frame| render::view(frame, &mut engine.state)) {
        error!("Failed to render initial frame: {}", e);
    }

    // Trigger startup discovery (non-blocking)
    spawn::spawn_tool_availability_check(engine.msg_sender());

    // Dispatch based on auto-start detection
    dispatch_startup_action(&mut engine, startup_result);

    // Run the main loop
    let result = run_loop(&mut term, &mut engine, &mut *clipboard);

    // Disable mouse capture FIRST: stops the terminal from generating new
    // SGR mouse reports before we drain the TTY queue.
    terminal::disable_mouse_capture();

    // Drain any mouse SGR reports that were already buffered in the kernel
    // TTY queue before DisableMouseCapture took effect. Without this drain,
    // those bytes remain in the queue and the shell reads them after exit,
    // printing garbage on the command line.
    event::drain_input(std::time::Duration::from_millis(50));

    // Shutdown engine (stops watcher, cleans up sessions). Safe to call now —
    // no new SGR sequences will queue because capture is already disabled.
    engine.shutdown().await;

    // Restore terminal (TUI-specific)
    ratatui::restore();

    result
}

/// Run TUI without Flutter (for testing/demo)
pub async fn run() -> Result<()> {
    // Create engine with dummy path
    let dummy_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let mut engine = Engine::new(dummy_path);

    // Demo mode: use NullClipboard — no display server may be available and
    // this is not a user-facing entry point.
    let mut clipboard: Box<dyn Clipboard> = Box::new(NullClipboard);

    // Demo mode does not enable mouse capture — settings are dummy values
    // and the path is not a user-facing entry point.
    let mut term = ratatui::init();

    // Install panic hook AFTER ratatui::init() for consistent ordering with
    // the other entry points (see run_with_project for full rationale).
    terminal::install_panic_hook();

    // Run the main loop
    let result = run_loop(&mut term, &mut engine, &mut *clipboard);

    // Shutdown engine
    engine.shutdown().await;

    // Restore terminal
    ratatui::restore();
    result
}

/// Dispatch the startup action returned by [`startup::startup_flutter`].
///
/// Auto-start sends `StartAutoLaunch` (which internally triggers device
/// discovery and auto-launches the session). Ready state triggers device
/// discovery directly so the NewSessionDialog is populated.
///
/// # Ordering
///
/// `process_message` is called synchronously before `run_loop` starts.
/// This is safe because `StartAutoLaunch` returns an `UpdateAction` (async
/// side effect), not a follow-up `Message`. If the handler is changed to
/// return a follow-up `Message`, this call site must switch to
/// `engine.msg_sender().try_send()` to preserve ordering.
fn dispatch_startup_action(engine: &mut Engine, action: startup::StartupAction) {
    match action {
        startup::StartupAction::AutoStart { configs } => {
            // Auto-start detected: send StartAutoLaunch which triggers device
            // discovery and auto-launches the session. spawn_device_discovery()
            // is NOT called here — the StartAutoLaunch handler dispatches
            // DiscoverDevicesAndAutoLaunch internally.
            let cache_allowed = engine.settings.behavior.auto_launch;
            engine.process_message(Message::StartAutoLaunch {
                configs,
                cache_allowed,
            });
        }
        startup::StartupAction::Ready => {
            // No auto-start — discover devices for the NewSessionDialog
            if let Some(flutter) = engine.state.flutter_executable() {
                spawn::spawn_device_discovery(engine.msg_sender(), flutter);
            } else {
                // SDK not found — clear the loading spinner with an error
                let _ = engine.msg_sender().try_send(Message::DeviceDiscoveryFailed {
                    error: "Flutter SDK not found. Configure sdk_path in .fdemon/config.toml or ensure flutter is on your PATH.".into(),
                    is_background: false,
                });
            }
        }
    }
}

/// Main event loop
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    engine: &mut Engine,
    clipboard: &mut dyn Clipboard,
) -> Result<()> {
    while !engine.should_quit() {
        // Drain and process all pending messages
        engine.drain_pending_messages();

        // Execute any runner-side-effect actions queued by the TEA update cycle.
        // `SetMouseCapture` and `WriteClipboard` require synchronous terminal /
        // clipboard I/O that must be performed here, in the runner, rather than
        // in the async `handle_action` dispatcher.
        handle_runner_actions(engine, clipboard);

        // Flush batched logs
        engine.flush_pending_logs();

        // Render
        terminal.draw(|frame| render::view(frame, &mut engine.state))?;

        // Handle terminal events (TUI-specific)
        if let Some(message) = event::poll()? {
            engine.process_message(message);
            // Handle any runner actions produced by the event (e.g. user
            // pressed Alt+m to toggle mouse capture, or right-clicked to copy).
            handle_runner_actions(engine, clipboard);
        }
    }

    Ok(())
}

/// Execute runner-side-effect actions drained from the engine's pending queue.
///
/// `UpdateAction::SetMouseCapture` — calls [`terminal::set_mouse_capture`] and,
/// on success, enqueues `Message::MouseCaptureChanged` so the TEA state reflects
/// the new terminal mode. On failure, pushes a warning toast and does NOT enqueue
/// a state-change message (the flag stays unchanged).
///
/// `UpdateAction::WriteClipboard` — calls [`Clipboard::write_text`] on the
/// runner-owned clipboard handle. On failure, pushes a warning toast.
///
/// All other action variants are silently ignored (they should not appear here;
/// `process.rs` only routes the two variants above to the pending queue).
pub(crate) fn handle_runner_actions(engine: &mut Engine, clipboard: &mut dyn Clipboard) {
    for action in engine.drain_runner_actions() {
        match action {
            UpdateAction::SetMouseCapture(target) => {
                match terminal::set_mouse_capture(target) {
                    Ok(()) => {
                        // Round-trip the state change through the message bus so
                        // the TEA update cycle updates `mouse_capture_active`.
                        // Use try_send: if the channel is full we log and move on —
                        // the state is slightly stale but the next poll will clear it.
                        let msg =
                            fdemon_app::message::Message::MouseCaptureChanged { active: target };
                        if let Err(e) = engine.msg_sender().try_send(msg) {
                            warn!("failed to enqueue MouseCaptureChanged: {e}");
                        }
                    }
                    Err(e) => {
                        warn!("set_mouse_capture({target}) failed: {e}");
                        engine.state.push_toast(
                            ToastLevel::Warn,
                            format!("Mouse capture toggle failed: {e}"),
                        );
                    }
                }
            }
            UpdateAction::WriteClipboard { text } => {
                if let Err(e) = clipboard.write_text(&text) {
                    warn!("clipboard write failed: {e}");
                    engine
                        .state
                        .push_toast(ToastLevel::Warn, format!("Clipboard write failed: {e}"));
                }
            }
            _ => {
                // Should not reach here — only SetMouseCapture and WriteClipboard
                // are pushed to pending_runner_actions by process.rs. Log at warn
                // if an unexpected variant slips through.
                warn!("unexpected runner action: {action:?}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fdemon_app::{Engine, UpdateAction};
    use fdemon_core::prelude::*;

    use super::handle_runner_actions;

    // ─── helpers ────────────────────────────────────────────────────────────────

    /// In-memory clipboard stub for runner tests.
    ///
    /// Mirrors `fdemon_app::services::MemoryClipboard` but is defined locally
    /// because `MemoryClipboard` is `#[cfg(test)]`-gated inside `fdemon-app` and
    /// is therefore not accessible to depending crates in their test builds.
    #[derive(Default)]
    struct LocalMemoryClipboard {
        pub writes: Vec<String>,
    }
    impl fdemon_app::services::Clipboard for LocalMemoryClipboard {
        fn write_text(&mut self, text: &str) -> Result<()> {
            self.writes.push(text.to_string());
            Ok(())
        }
    }

    /// A `Clipboard` impl whose `write_text` always returns an error.
    /// Used to test the failure-toast path in `handle_runner_actions`.
    struct FailingClipboard;
    impl fdemon_app::services::Clipboard for FailingClipboard {
        fn write_text(&mut self, _text: &str) -> Result<()> {
            Err(fdemon_core::Error::terminal("simulated clipboard error"))
        }
    }

    fn dummy_engine() -> Engine {
        let path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        Engine::new(path)
    }

    // ─── SetMouseCapture — success path ─────────────────────────────────────────

    /// When `set_mouse_capture` returns `Ok(())`, `handle_runner_actions` must
    /// enqueue a `MouseCaptureChanged` follow-up message on the engine's channel.
    ///
    /// `set_mouse_capture(false)` when the MOUSE_CAPTURE_ON flag is already
    /// `false` (the default in test environments) returns `Ok(())` via the
    /// idempotency guard — it is a no-op that still indicates success.
    /// The runner should enqueue `MouseCaptureChanged { active: false }`, and
    /// after `drain_pending_messages`, `mouse_capture_active` should be `false`.
    ///
    /// Note: `mouse_capture_active` starts as `true` in `AppState::with_settings`
    /// when `settings.ui.enable_mouse = true` (the default). After the handler
    /// processes `MouseCaptureChanged { active: false }`, it becomes `false`.
    ///
    /// The `MouseCaptureChanged` handler also pushes an informational toast, so
    /// we verify the toast level is Info (not Warn) to confirm the success path
    /// (not the failure-toast path) was taken.
    #[tokio::test]
    async fn test_set_mouse_capture_action_enqueues_followup_message() {
        let mut engine = dummy_engine();
        // set_mouse_capture(false) with MOUSE_CAPTURE_ON = false returns Ok(())
        // via idempotency, so the success branch executes without stdout writes.
        engine
            .state
            .pending_runner_actions
            .push(UpdateAction::SetMouseCapture(false));

        let mut clipboard = LocalMemoryClipboard::default();
        handle_runner_actions(&mut engine, &mut clipboard);

        // The engine's message channel should now contain MouseCaptureChanged.
        // drain_pending_messages() processes queued messages; after that,
        // `mouse_capture_active` should reflect the new state (false).
        engine.drain_pending_messages();
        assert!(
            !engine.state.mouse_capture_active,
            "mouse_capture_active should be false after MouseCaptureChanged(false)"
        );

        // The MouseCaptureChanged handler pushes an Info toast ("Mouse capture off…").
        // Verify the toast is Info (not Warn) — a Warn toast would indicate the
        // failure path in handle_runner_actions was taken instead of the success path.
        assert_eq!(
            engine.state.toasts.len(),
            1,
            "expected exactly one Info toast from MouseCaptureChanged handler"
        );
        assert_eq!(
            engine.state.toasts[0].level,
            fdemon_app::ToastLevel::Info,
            "toast must be Info (set_mouse_capture succeeded), not Warn (failure path)"
        );
    }

    // ─── WriteClipboard — success path ──────────────────────────────────────────

    /// `handle_runner_actions` for `WriteClipboard` must call `write_text` on
    /// the runner-owned clipboard and NOT push a toast on success.
    #[tokio::test]
    async fn test_write_clipboard_action_writes_to_clipboard() {
        let mut engine = dummy_engine();
        engine
            .state
            .pending_runner_actions
            .push(UpdateAction::WriteClipboard {
                text: "hello clipboard".to_string(),
            });

        let mut clipboard = LocalMemoryClipboard::default();
        handle_runner_actions(&mut engine, &mut clipboard);

        assert_eq!(
            clipboard.writes,
            vec!["hello clipboard"],
            "expected one write with the correct text"
        );
        assert!(
            engine.state.toasts.is_empty(),
            "no toasts expected on successful clipboard write"
        );
    }

    // ─── WriteClipboard — failure path ──────────────────────────────────────────

    /// When `write_text` returns an error, `handle_runner_actions` must push a
    /// warning toast and NOT panic.
    #[tokio::test]
    async fn test_write_clipboard_failure_pushes_warning_toast() {
        let mut engine = dummy_engine();
        engine
            .state
            .pending_runner_actions
            .push(UpdateAction::WriteClipboard {
                text: "will fail".to_string(),
            });

        let mut clipboard = FailingClipboard;
        handle_runner_actions(&mut engine, &mut clipboard);

        assert_eq!(
            engine.state.toasts.len(),
            1,
            "expected exactly one warning toast on clipboard write failure"
        );
        let toast = &engine.state.toasts[0];
        assert!(
            toast.text.contains("Clipboard write failed"),
            "toast text should describe the failure, got: {}",
            toast.text
        );
    }
}
