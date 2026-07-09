//! Main TUI runner - entry points and event loop
//!
//! Contains the core application lifecycle:
//! - `run_with_project`: Main entry point with Flutter project
//! - `run_with_project_and_dap`: Like `run_with_project` but with DAP port override and auto-start
//! - `run_with_engine`: Embedder entry point - runs the TUI on a caller-constructed `Engine`
//! - `run`: Demo/test entry point without Flutter
//! - `run_loop`: Main event loop processing terminal and daemon events

use std::path::Path;

use tracing::{error, warn};

use fdemon_app::config::should_auto_start_dap;
use fdemon_app::install_wizard::WizardOrigin;
use fdemon_app::message::Message;
use fdemon_app::services::{create_clipboard, Clipboard, NullClipboard};
use fdemon_app::spawn;
use fdemon_app::{Engine, ToastLevel, UpdateAction};
use fdemon_core::prelude::*;

use crate::{event, render, startup, terminal};

/// DAP startup options for [`run_with_engine`].
///
/// Mirrors the CLI surface of [`run_with_project_and_dap`]:
/// - `port` carries the `--dap-port` override (forces `dap.enabled = true`).
/// - `parent_ide` carries the `--dap-config` IDE override (bypasses
///   environment-based detection).
///
/// Passing `Some(DapOptions::default())` (both fields `None`) still evaluates
/// [`should_auto_start_dap`] from settings, exactly like
/// `run_with_project_and_dap(path, None, None)`. Passing `None` to
/// [`run_with_engine`] skips DAP startup entirely, exactly like
/// [`run_with_project`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DapOptions {
    /// `--dap-port` CLI override: sets `settings.dap.port` and forces
    /// `settings.dap.enabled = true`.
    pub port: Option<u16>,
    /// `--dap-config` IDE override: stored on `AppState` so IDE config
    /// generation bypasses environment-based detection.
    pub parent_ide: Option<fdemon_app::config::ParentIde>,
}

/// Run the TUI application with a Flutter project
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Create the engine (handles all shared initialization)
    let engine = Engine::new(project_path.to_path_buf());

    // No DAP options: skip DAP startup entirely (legacy non-DAP entry point).
    run_with_engine(engine, None).await
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
    let engine = Engine::new(project_path.to_path_buf());

    run_with_engine(
        engine,
        Some(DapOptions {
            port: dap_port,
            parent_ide: dap_config,
        }),
    )
    .await
}

/// Run the TUI event loop on a caller-constructed [`Engine`].
///
/// This is the embedder entry point: a host binary that needs to share one
/// `Engine` between the TUI and other in-process consumers (e.g. an MCP
/// server) constructs the `Engine` itself, attaches subscribers, services, or
/// plugins, and then hands it here. [`run_with_project`] and
/// [`run_with_project_and_dap`] both delegate to this function.
///
/// # Pre-call contract
///
/// The engine MUST come from [`Engine::new`], which performs all shared
/// initialization the TUI relies on: `.fdemon` directory init, settings load,
/// Flutter SDK resolution, signal handler spawn, and file-watcher start.
/// Between `Engine::new` and this call, embedders may:
/// - subscribe to engine events via [`Engine::subscribe`],
/// - take service handles (`log_service`, `state_service`, ...),
/// - register plugins via [`Engine::register_plugin`] — this function calls
///   [`Engine::notify_plugins_start`] before entering the event loop.
///
/// Embedders must NOT pre-initialize the terminal or process messages that
/// return follow-up `Message`s (the channel is drained only once the loop
/// starts).
///
/// # DAP startup
///
/// - `dap: None` — skip DAP startup entirely (matches [`run_with_project`]).
/// - `dap: Some(opts)` — apply the CLI overrides in `opts`, then evaluate
///   [`should_auto_start_dap`] and send `Message::StartDapServer` if it
///   returns `true` (matches [`run_with_project_and_dap`]).
///
/// # Lifecycle
///
/// Takes ownership of the engine and consumes it: on exit (quit or error) the
/// terminal is restored and [`Engine::shutdown`] is awaited (stops the
/// watcher, cleans up sessions).
pub async fn run_with_engine(mut engine: Engine, dap: Option<DapOptions>) -> Result<()> {
    if let Some(dap) = dap {
        apply_dap_overrides(&mut engine, dap);

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
    }

    // Initialize clipboard: backend chosen from config + environment (OS
    // clipboard on desktop, OSC 52 over SSH/headless). Pushes a warning toast
    // when no backend can work so the user knows before attempting to copy.
    let mut clipboard = init_clipboard(&mut engine);

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
    if engine.settings.behavior.should_run_version_check() {
        spawn::spawn_version_check(
            engine.msg_sender(),
            std::time::Duration::from_secs(
                engine.settings.behavior.version_check_timeout_secs as u64,
            ),
        );
    }

    // Dispatch based on auto-start detection
    dispatch_startup_action(&mut engine, startup_result);

    // Notify plugins the engine is about to enter the event loop. No-op when
    // no plugins are registered (the `run_with_project*` paths); embedders
    // that registered plugins before calling get the documented on_start
    // lifecycle callback here.
    engine.notify_plugins_start();

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

/// Apply the CLI-style DAP overrides carried by [`DapOptions`] to the engine.
///
/// Split from [`run_with_engine`] so the override plumbing is unit-testable
/// without entering the terminal event loop. Does NOT evaluate
/// [`should_auto_start_dap`] — the caller decides whether auto-start fires.
fn apply_dap_overrides(engine: &mut Engine, dap: DapOptions) {
    // Apply --dap-port CLI override: sets port and forces enabled = true in
    // both settings copies, keeping them in sync.
    if let Some(port) = dap.port {
        engine.apply_cli_dap_override(port);
    }

    // Apply --dap-config IDE override: stored on AppState so handle_started()
    // can pass it to GenerateIdeConfig, bypassing environment-based detection.
    if let Some(ide) = dap.parent_ide {
        engine.apply_cli_dap_config_override(ide);
    }
}

/// Select and construct the clipboard backend for this session.
///
/// Delegates backend choice to [`create_clipboard`] (config `ui.clipboard_mode`
/// plus environment detection: OS clipboard on desktop sessions, OSC 52 escape
/// sequences over SSH or without a display server). When no backend can work,
/// pushes a startup warning toast so the user knows before attempting to copy;
/// writes on the returned clipboard then fail, firing the runner's
/// failure-toast path.
fn init_clipboard(engine: &mut Engine) -> Box<dyn Clipboard> {
    let init = create_clipboard(engine.settings.ui.clipboard_mode);
    if let Some(reason) = init.unavailable_reason {
        engine.state.push_toast(
            ToastLevel::Warn,
            format!("Clipboard unavailable; copy is disabled ({reason})"),
        );
    }
    init.clipboard
}

/// Dispatch the startup action returned by [`startup::startup_flutter`].
///
/// Auto-start sends `StartAutoLaunch` (which internally triggers device
/// discovery and auto-launches the session). Ready state triggers device
/// discovery directly so the NewSessionDialog is populated.
///
/// # No-SDK early exit
///
/// When `flutter_executable()` is `None`, both startup paths would otherwise
/// dead-end (AutoStart silently no-ops; Ready shows an empty dialog).  The
/// early return opens the diagnostics wizard regardless of which path was
/// chosen, giving the user actionable feedback.
///
/// # Ordering
///
/// `process_message` is called synchronously before `run_loop` starts.
/// This is safe because `StartAutoLaunch` returns an `UpdateAction` (async
/// side effect), not a follow-up `Message`. If the handler is changed to
/// return a follow-up `Message`, this call site must switch to
/// `engine.msg_sender().try_send()` to preserve ordering.
fn dispatch_startup_action(engine: &mut Engine, action: startup::StartupAction) {
    // No resolvable SDK: open the diagnostics wizard from either startup path
    // instead of a dead-end (Ready) or a silent no-op (AutoStart).
    // Use try_send so a saturated channel does not block startup.
    if engine.state.flutter_executable().is_none() {
        let _ = engine.msg_sender().try_send(Message::ShowInstallWizard {
            origin: WizardOrigin::Bootstrap,
        });
        return;
    }

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
            // flutter_executable() is Some here (None was handled above).
            // Discover devices so the NewSessionDialog is populated.
            if let Some(flutter) = engine.state.flutter_executable() {
                spawn::spawn_device_discovery(engine.msg_sender(), flutter);
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

        // Render.
        //
        // This redraws on EVERY loop iteration (driven by the ~50 ms `Tick` from
        // `event::poll`), not only when state changes. That unconditional cadence
        // is load-bearing for time-based animations that are pure functions of
        // wall-clock `now` — the reload-success header flash, the shimmer, and
        // the spinner all decay/advance across frames with no state mutation
        // between them. ratatui's double-buffer diff already suppresses redundant
        // terminal writes, so the steady state stays cheap. Do NOT gate this
        // `draw` on a dirty/needs-redraw flag without adding a compensating
        // animation-redraw trigger, or those effects will freeze mid-animation.
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
/// the new terminal mode.  If the channel is full (saturated), state is mutated
/// directly and a warn toast is pushed (deliberate TEA exception — see inline
/// comment).  On terminal I/O failure, pushes a warning toast and does NOT
/// enqueue a state-change message (the flag stays unchanged).
///
/// `UpdateAction::WriteClipboard` — calls [`Clipboard::write_text`] on the
/// runner-owned clipboard handle. On failure, pushes a warning toast.
///
/// All other `UpdateAction` variants are explicitly enumerated in a non-runner
/// arm and emit a warn! if they unexpectedly arrive here.  The exhaustive match
/// ensures the compiler requires an explicit decision whenever a new variant is
/// added to the enum.
pub(crate) fn handle_runner_actions(engine: &mut Engine, clipboard: &mut dyn Clipboard) {
    for action in engine.drain_runner_actions() {
        match action {
            UpdateAction::SetMouseCapture(target) => {
                match terminal::set_mouse_capture(target) {
                    Ok(()) => {
                        // Round-trip the state change through the message bus so
                        // the TEA update cycle updates `mouse_capture_active`.
                        // Use try_send: if the channel is full we fall back to
                        // direct state mutation so the model never lies about the
                        // terminal mode.
                        let msg =
                            fdemon_app::message::Message::MouseCaptureChanged { active: target };
                        if let Err(e) = engine.msg_sender().try_send(msg) {
                            // Channel is saturated. The MouseCaptureChanged handler
                            // would have set state.mouse_capture_active = target and
                            // pushed a status toast. Apply those side effects directly
                            // here so the model does not lie about the terminal state.
                            //
                            // Direct state mutation from the runner is a deliberate
                            // exception to the TEA "single update site" rule, justified
                            // because we are reflecting an already-observed terminal
                            // state change that the message would have applied if the
                            // channel had capacity.
                            error!(
                                "MouseCaptureChanged channel full; applying state directly: {e}"
                            );
                            // Shared helper: also clears an in-flight drag on
                            // capture-off, exactly like the handler would have.
                            engine.state.set_mouse_capture_active(target);
                            engine.state.push_toast(
                                ToastLevel::Warn,
                                if target {
                                    "Mouse capture on (channel full; state applied directly)"
                                } else {
                                    "Mouse capture off (channel full; state applied directly)"
                                }
                                .to_string(),
                            );
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
            // ── Non-runner variants ──────────────────────────────────────────────
            // These variants are handled by process.rs::handle_action and should
            // NEVER arrive in the runner queue. If one does it indicates a routing
            // bug in process.rs; warn but do not panic so the user can continue.
            //
            // Compile-time note: when adding a new UpdateAction variant, decide
            // whether it belongs to the runner queue (add an arm above) or the
            // process.rs queue (add it to this list below).
            UpdateAction::SpawnTask(..)
            | UpdateAction::DiscoverDevices { .. }
            | UpdateAction::RefreshDevicesBackground { .. }
            | UpdateAction::RefreshDevicesAndBootableBackground { .. }
            | UpdateAction::DiscoverDevicesAndBootable { .. }
            | UpdateAction::DiscoverDevicesAndAutoLaunch { .. }
            | UpdateAction::DiscoverEmulators { .. }
            | UpdateAction::LaunchEmulator { .. }
            | UpdateAction::LaunchIOSSimulator
            | UpdateAction::SpawnSession { .. }
            | UpdateAction::ReloadAllSessions { .. }
            | UpdateAction::CheckToolAvailability
            | UpdateAction::DiscoverBootableDevices
            | UpdateAction::BootDevice { .. }
            | UpdateAction::StartQrPairing { .. }
            | UpdateAction::AutoSaveConfig { .. }
            | UpdateAction::PersistSettings { .. }
            | UpdateAction::LaunchFlutterSession { .. }
            | UpdateAction::DiscoverEntryPoints { .. }
            | UpdateAction::ConnectVmService { .. }
            | UpdateAction::StartPerformanceMonitoring { .. }
            | UpdateAction::FetchWidgetTree { .. }
            | UpdateAction::FetchLayoutData { .. }
            | UpdateAction::FetchInspectorProperties { .. }
            | UpdateAction::ToggleOverlay { .. }
            | UpdateAction::OpenBrowserDevTools { .. }
            | UpdateAction::StartNetworkMonitoring { .. }
            | UpdateAction::FetchHttpRequestDetail { .. }
            | UpdateAction::ClearHttpProfile { .. }
            | UpdateAction::DisposeDevToolsGroups { .. }
            | UpdateAction::PauseIsolate { .. }
            | UpdateAction::ResumeIsolate { .. }
            | UpdateAction::AddBreakpoint { .. }
            | UpdateAction::RemoveBreakpoint { .. }
            | UpdateAction::SetIsolatePauseMode { .. }
            | UpdateAction::SpawnDapServer { .. }
            | UpdateAction::StopDapServer
            | UpdateAction::ForwardDapDebugEvents(..)
            | UpdateAction::GenerateIdeConfig { .. }
            | UpdateAction::StartNativeLogCapture { .. }
            | UpdateAction::SpawnPreAppSources { .. }
            | UpdateAction::RunToolchainPreflight { .. }
            | UpdateAction::RunWizardStep { .. }
            | UpdateAction::FetchFlutterReleaseManifest
            | UpdateAction::ScanInstalledSdks { .. }
            | UpdateAction::SwitchFlutterVersion { .. }
            | UpdateAction::RemoveFlutterVersion { .. }
            | UpdateAction::ProbeFlutterVersion { .. }
            | UpdateAction::SendDaemonCommand { .. }
            | UpdateAction::StartTimelineMonitoring { .. }
            | UpdateAction::ToggleProfileWidgetBuilds { .. }
            | UpdateAction::FetchWidgetLocationIdMap { .. }
            | UpdateAction::DebounceFrameAnchor { .. } => {
                warn!("runner action queue received non-runner variant: {action:?}");
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

    // ─── SetMouseCapture — channel-full fallback ─────────────────────────────────

    /// When the message channel is full and `try_send(MouseCaptureChanged)` fails,
    /// `handle_runner_actions` must mutate `state.mouse_capture_active` directly
    /// and push a `ToastLevel::Warn` toast so the model does not lie about the
    /// terminal state.
    ///
    /// Strategy: drain the channel capacity by stuffing it with dummy messages,
    /// then exercise `SetMouseCapture(true)` so `try_send` fails, then verify
    /// both direct state and toast side-effects were applied.
    ///
    /// Note: `set_mouse_capture(false)` in test environments uses an idempotency
    /// guard and returns `Ok(())` without touching the real TTY. We use `false`
    /// here so the runner succeeds at the terminal level but the channel is full,
    /// forcing the direct-mutation path.
    #[tokio::test]
    async fn test_mouse_capture_changed_channel_full_applies_state_directly() {
        let mut engine = dummy_engine();

        // Fill the channel to capacity (default is 256 for the engine channel).
        // We saturate it by sending as many messages as the channel accepts.
        let sender = engine.msg_sender();
        let capacity = 256usize;
        for _ in 0..capacity {
            // Tick is a simple no-payload message — safe to use here without
            // triggering side effects that would interfere with the test.
            let _ = sender.try_send(fdemon_app::message::Message::Tick);
        }

        // The mouse_capture_active state starts as true (enable_mouse = true by default).
        // We will set it to false via SetMouseCapture to verify the direct-mutation path.
        // First set it to true explicitly so the transition is observable.
        engine.state.mouse_capture_active = true;

        // Install an in-flight drag on a selected session: capture-off through
        // the fallback must clear it exactly like the normal handler would
        // (an orphaned drag would edge auto-scroll forever).
        engine
            .state
            .session_manager
            .create_session(&crate::test_utils::test_device("d1", "iPhone"))
            .unwrap();
        {
            use fdemon_app::log_view_state::{LogSelection, SelPoint};
            let handle = engine.state.session_manager.selected_mut().unwrap();
            handle.session.log_view_state.selection = Some(LogSelection::new(SelPoint {
                entry_id: 1,
                frame_index: None,
                col: 0,
            }));
        }

        // Push a SetMouseCapture(false) action — terminal succeeds (idempotency guard),
        // but try_send should fail because the channel is full.
        engine
            .state
            .pending_runner_actions
            .push(UpdateAction::SetMouseCapture(false));

        let mut clipboard = LocalMemoryClipboard::default();
        handle_runner_actions(&mut engine, &mut clipboard);

        // The direct-mutation path should have set mouse_capture_active = false.
        assert!(
            !engine.state.mouse_capture_active,
            "mouse_capture_active must be false after channel-full direct mutation"
        );

        // ...and cleared the in-flight drag (shared helper with the handler).
        assert!(
            engine
                .state
                .session_manager
                .selected()
                .unwrap()
                .session
                .log_view_state
                .selection
                .is_none(),
            "channel-full capture-off must clear an in-flight drag-selection"
        );

        // A Warn toast should have been pushed (not an Info toast, which would indicate
        // the normal success path via try_send was taken instead).
        assert_eq!(
            engine.state.toasts.len(),
            1,
            "expected exactly one Warn toast from channel-full fallback"
        );
        assert_eq!(
            engine.state.toasts[0].level,
            fdemon_app::ToastLevel::Warn,
            "channel-full fallback must push a Warn toast, not Info"
        );
        assert!(
            engine.state.toasts[0].text.contains("channel full"),
            "toast must mention 'channel full', got: {}",
            engine.state.toasts[0].text
        );
    }

    // ─── NullClipboard — right-click produces failure toast ──────────────────────

    /// When `NullClipboard` is the active clipboard and `WriteClipboard` is
    /// dispatched, `handle_runner_actions` must push a `ToastLevel::Warn` toast
    /// containing "Clipboard write failed". This verifies that the NullClipboard
    /// adoption at runner fallback sites produces the correct UX signal.
    #[tokio::test]
    async fn test_null_clipboard_returns_err_and_runner_pushes_toast() {
        let mut engine = dummy_engine();
        engine
            .state
            .pending_runner_actions
            .push(UpdateAction::WriteClipboard {
                text: "some text to copy".to_string(),
            });

        // Use NullClipboard — simulates the clipboard-unavailable fallback path.
        let mut clipboard = fdemon_app::services::NullClipboard;
        handle_runner_actions(&mut engine, &mut clipboard);

        assert_eq!(
            engine.state.toasts.len(),
            1,
            "expected exactly one Warn toast when NullClipboard write fails"
        );
        let toast = &engine.state.toasts[0];
        assert_eq!(
            toast.level,
            fdemon_app::ToastLevel::Warn,
            "NullClipboard failure must produce a Warn toast"
        );
        assert!(
            toast.text.contains("Clipboard write failed"),
            "toast must mention 'Clipboard write failed', got: {}",
            toast.text
        );
    }

    // ─── apply_dap_overrides — run_with_engine DAP plumbing ──────────────────

    /// `DapOptions { port: Some(_) }` must set the DAP port and force
    /// `enabled = true` in BOTH settings copies (engine cache and AppState),
    /// matching the historical `--dap-port` handling in
    /// `run_with_project_and_dap`.
    #[tokio::test]
    async fn test_apply_dap_overrides_port_sets_enabled_in_both_settings() {
        let mut engine = dummy_engine();
        assert!(
            !engine.settings.dap.enabled,
            "test precondition: DAP must be disabled by default"
        );

        super::apply_dap_overrides(
            &mut engine,
            super::DapOptions {
                port: Some(4711),
                parent_ide: None,
            },
        );

        assert_eq!(engine.settings.dap.port, 4711);
        assert!(engine.settings.dap.enabled);
        assert_eq!(engine.state.settings.dap.port, 4711);
        assert!(engine.state.settings.dap.enabled);
    }

    /// `DapOptions { parent_ide: Some(_) }` must store the IDE override on
    /// `AppState.cli_dap_config_override`, matching the historical
    /// `--dap-config` handling in `run_with_project_and_dap`.
    #[tokio::test]
    async fn test_apply_dap_overrides_parent_ide_stored_on_state() {
        use fdemon_app::config::ParentIde;

        let mut engine = dummy_engine();
        assert_eq!(engine.state.cli_dap_config_override, None);

        super::apply_dap_overrides(
            &mut engine,
            super::DapOptions {
                port: None,
                parent_ide: Some(ParentIde::Zed),
            },
        );

        assert_eq!(engine.state.cli_dap_config_override, Some(ParentIde::Zed));
        assert!(
            !engine.settings.dap.enabled,
            "parent_ide alone must not force dap.enabled"
        );
    }

    /// `DapOptions::default()` (both fields `None`) must leave the engine's
    /// DAP settings and state untouched — equivalent to
    /// `run_with_project_and_dap(path, None, None)` before the auto-start
    /// evaluation.
    #[tokio::test]
    async fn test_apply_dap_overrides_default_is_noop() {
        let mut engine = dummy_engine();
        let port_before = engine.settings.dap.port;
        let enabled_before = engine.settings.dap.enabled;

        super::apply_dap_overrides(&mut engine, super::DapOptions::default());

        assert_eq!(engine.settings.dap.port, port_before);
        assert_eq!(engine.settings.dap.enabled, enabled_before);
        assert_eq!(engine.state.cli_dap_config_override, None);
    }

    // ─── dispatch_startup_action — no-SDK wizard hook ────────────────────────

    /// When `flutter_executable()` is `None`, `dispatch_startup_action` must
    /// enqueue `ShowInstallWizard` for the `Ready` startup path and, after
    /// `drain_pending_messages`, transition to `UiMode::InstallWizard`.
    ///
    /// The no-SDK precondition is forced explicitly (`resolved_sdk = None`) so the
    /// test is deterministic on hosts where SDK detection would otherwise resolve
    /// a real Flutter (e.g. an SDK in the fvm versions cache).
    #[tokio::test]
    async fn test_dispatch_startup_ready_no_sdk_opens_wizard() {
        use fdemon_app::state::UiMode;

        let mut engine = dummy_engine();
        // Force the no-SDK precondition regardless of the host environment.
        engine.state.resolved_sdk = None;
        assert!(
            engine.state.flutter_executable().is_none(),
            "test engine must have no resolved SDK"
        );

        super::dispatch_startup_action(&mut engine, super::startup::StartupAction::Ready);

        // Drain messages so ShowInstallWizard is processed
        engine.drain_pending_messages();

        assert_eq!(
            engine.state.ui_mode,
            UiMode::InstallWizard,
            "Ready + no SDK must transition to UiMode::InstallWizard"
        );
    }

    /// When `flutter_executable()` is `None`, `dispatch_startup_action` must
    /// enqueue `ShowInstallWizard` for the `AutoStart` path as well.
    ///
    /// Previously `AutoStart` silently no-op'd when there was no SDK — this
    /// test verifies the silent dead-end is closed.
    #[tokio::test]
    async fn test_dispatch_startup_autostart_no_sdk_opens_wizard() {
        use fdemon_app::config::LoadedConfigs;
        use fdemon_app::state::UiMode;

        let mut engine = dummy_engine();
        // Force the no-SDK precondition regardless of the host environment.
        engine.state.resolved_sdk = None;
        assert!(
            engine.state.flutter_executable().is_none(),
            "test engine must have no resolved SDK"
        );

        let configs = LoadedConfigs::default();
        super::dispatch_startup_action(
            &mut engine,
            super::startup::StartupAction::AutoStart { configs },
        );

        engine.drain_pending_messages();

        assert_eq!(
            engine.state.ui_mode,
            UiMode::InstallWizard,
            "AutoStart + no SDK must transition to UiMode::InstallWizard"
        );
    }
}
