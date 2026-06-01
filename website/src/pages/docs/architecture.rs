use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};

use crate::components::code_block::CodeBlock;
use crate::components::diagrams::*;
use crate::components::icons::*;

#[component]
pub fn Architecture() -> impl IntoView {
    view! {
        <Title text="Architecture" />
        <Meta name="description" content="Flutter Demon's internal architecture: a 5-crate Cargo workspace following The Elm Architecture (TEA). Covers the Engine, session manager, DevTools subsystem, and DAP server." />
        <Link rel="canonical" href="https://fdemon.dev/docs/architecture" />
        <div class="animate-fade-in space-y-8">
            <h1 class="text-4xl font-bold text-white">"Architecture"</h1>
            <p class="text-lg text-slate-400">
                "Flutter Demon is organized as a Cargo workspace with five library crates and one binary. "
                "The application follows The Elm Architecture (TEA) for predictable state management, "
                "with compile-time enforced layer boundaries between crates."
            </p>

            // ── System Architecture ─────────────────────────
            <Section title="System Architecture">
                <DiagramContainer title="Crate Dependency Graph">
                    <ArchNode
                        title="flutter-demon (binary)"
                        subtitle="CLI parsing, headless NDJSON mode"
                        color=NodeColor::Slate
                        icon=|| view! { <Terminal class="w-4 h-4" /> }.into_any()
                    />
                    <FlowArrow />
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-3 mx-[12%] md:mx-[16%]">
                        <ArchNode
                            title="fdemon-tui"
                            subtitle="Ratatui terminal UI, widgets, runner"
                            color=NodeColor::Purple
                            icon=|| view! { <Layout class="w-4 h-4" /> }.into_any()
                        />
                        <ArchNode
                            title="fdemon-app"
                            subtitle="TEA engine, AppState, handler::update(), services, config"
                            color=NodeColor::Blue
                            icon=|| view! { <Layers class="w-4 h-4" /> }.into_any()
                        />
                    </div>
                    <FlowArrow />
                    <div class="grid grid-cols-1 md:grid-cols-3 gap-3 mx-[12%] md:mx-[16%]">
                        <ArchNode
                            title="fdemon-daemon"
                            subtitle="Flutter process I/O, JSON-RPC, device discovery, native logs"
                            color=NodeColor::Cyan
                            icon=|| view! { <Cpu class="w-4 h-4" /> }.into_any()
                        />
                        <ArchNode
                            title="fdemon-dap"
                            subtitle="Debug Adapter Protocol server, adapter, TCP/stdio transport"
                            color=NodeColor::Orange
                            icon=|| view! { <Zap class="w-4 h-4" /> }.into_any()
                        />
                        <ArchNode
                            title="fdemon-core"
                            subtitle="Domain types, events, discovery, error handling (zero deps)"
                            color=NodeColor::Green
                            icon=|| view! { <Database class="w-4 h-4" /> }.into_any()
                        />
                    </div>
                    <FlowArrow />
                    <ArchNode
                        title="Flutter Process"
                        subtitle="flutter run --machine"
                        color=NodeColor::Rose
                        icon=|| view! { <Smartphone class="w-4 h-4" /> }.into_any()
                    />
                </DiagramContainer>
            </Section>

            // ── The Elm Architecture ────────────────────────
            <Section title="The Elm Architecture (TEA)">
                <p class="text-slate-400">
                    "Flutter Demon follows the "<strong class="text-white">"TEA pattern"</strong>
                    " (Model-View-Update) for predictable state management."
                </p>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-blue-400 mb-2">"The Pattern"</h4>
                        <ul class="space-y-2 text-sm text-slate-400">
                            <li><strong class="text-white">"Model"</strong>" \u{2014} "<code class="text-blue-400">"AppState"</code>" holds the complete application state"</li>
                            <li><strong class="text-white">"Messages"</strong>" \u{2014} "<code class="text-blue-400">"Message"</code>" enum defines all possible events"</li>
                            <li><strong class="text-white">"Update"</strong>" \u{2014} "<code class="text-blue-400">"handler::update()"</code>" pure function: (State, Msg) \u{2192} (State, Option\u{3C}Action\u{3E})"</li>
                            <li><strong class="text-white">"View"</strong>" \u{2014} "<code class="text-blue-400">"tui::render()"</code>" renders state to the terminal"</li>
                        </ul>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-green-400 mb-2">"Benefits"</h4>
                        <ul class="space-y-2 text-sm text-slate-400">
                            <li>"\u{2713} Predictable state transitions"</li>
                            <li>"\u{2713} Easy testing (update is a pure function)"</li>
                            <li>"\u{2713} Clear separation of concerns"</li>
                            <li>"\u{2713} Time-travel debugging potential"</li>
                        </ul>
                    </div>
                </div>

                <DiagramContainer title="TEA Event Loop">
                    <div class="grid grid-cols-3 gap-2">
                        <ArchNode
                            title="Terminal"
                            subtitle="Keyboard events"
                            color=NodeColor::Purple
                            icon=|| view! { <Keyboard class="w-3 h-3" /> }.into_any()
                        />
                        <ArchNode
                            title="Daemon"
                            subtitle="Flutter process events"
                            color=NodeColor::Cyan
                            icon=|| view! { <Cpu class="w-3 h-3" /> }.into_any()
                        />
                        <ArchNode
                            title="Watcher / Timer"
                            subtitle="File changes, ticks"
                            color=NodeColor::Yellow
                            icon=|| view! { <Eye class="w-3 h-3" /> }.into_any()
                        />
                    </div>

                    <FlowArrow label="generates" />

                    <ArchNode
                        title="Message"
                        subtitle="Typed event enum (Key, Daemon, Tick, HotReload, ...)"
                        color=NodeColor::Blue
                        icon=|| view! { <Zap class="w-3 h-3" /> }.into_any()
                    />

                    <FlowArrow />

                    <ArchNode
                        title="handler::update(state, message)"
                        subtitle="Pure function \u{2192} (AppState, Option\u{3C}UpdateAction\u{3E})"
                        color=NodeColor::Orange
                        icon=|| view! { <RefreshCw class="w-3 h-3" /> }.into_any()
                    />

                    <div class="grid grid-cols-2 gap-3 mt-1">
                        <div>
                            <FlowArrow label="new state" />
                            <ArchNode
                                title="render(state)"
                                subtitle="State \u{2192} Terminal UI"
                                color=NodeColor::Purple
                                icon=|| view! { <Layout class="w-3 h-3" /> }.into_any()
                            />
                        </div>
                        <div>
                            <FlowArrow label="action" />
                            <ArchNode
                                title="Async Tasks"
                                subtitle="Side effects (reload, spawn, discover)"
                                color=NodeColor::Green
                                icon=|| view! { <Zap class="w-3 h-3" /> }.into_any()
                            />
                            <div class="text-center mt-2">
                                <span class="text-xs text-slate-500 italic">"\u{21A9} generates new Messages"</span>
                            </div>
                        </div>
                    </div>
                </DiagramContainer>
            </Section>

            // ── Layer Dependencies ──────────────────────────
            <Section title="Layer Dependencies">
                <p class="text-slate-400 mb-4">
                    "Each crate has clear responsibilities. Dependencies flow downward — lower crates never depend on higher ones. "
                    "Cargo enforces these boundaries at compile time."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Crate"</th>
                                <th class="p-4 font-medium">"Responsibility"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Depends on"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <LayerRow layer="flutter-demon" resp="CLI, entry point, headless mode" deps="All crates" />
                            <LayerRow layer="fdemon-tui" resp="Terminal UI presentation" deps="fdemon-core, fdemon-app" />
                            <LayerRow layer="fdemon-app" resp="TEA engine, state, orchestration, services, config" deps="fdemon-core, fdemon-daemon, fdemon-dap" />
                            <LayerRow layer="fdemon-dap" resp="DAP protocol, adapter, TCP/stdio transport" deps="fdemon-core" />
                            <LayerRow layer="fdemon-daemon" resp="Flutter process I/O, device/emulator discovery" deps="fdemon-core" />
                            <LayerRow layer="fdemon-core" resp="Domain types, events, discovery, error handling" deps="None" />
                        </tbody>
                    </table>
                </div>
                <p class="text-sm text-slate-500 mt-2">
                    "The TUI crate depends on App because of the TEA pattern: "
                    <code class="text-blue-400">"render()"</code>" must receive "<code class="text-blue-400">"AppState"</code>
                    " to render it. This is the fundamental TEA contract: View: State \u{2192} UI."
                </p>
            </Section>

            // ── Error Handling ───────────────────────────────
            <Section title="Error Handling">
                <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                    <div class="flex items-center gap-2 mb-3">
                        <Shield class="w-4 h-4 text-orange-400" />
                        <h4 class="font-bold text-white">"Error Classification"</h4>
                    </div>
                    <ul class="space-y-2 text-sm text-slate-400">
                        <li>"Custom "<code class="text-blue-400">"Error"</code>" enum with domain-specific variants (defined in "<code class="text-blue-400">"crates/fdemon-core/src/error.rs"</code>")"</li>
                        <li><code class="text-blue-400">"Result<T>"</code>" type alias used throughout the codebase"</li>
                        <li>"Errors are categorized as "<strong class="text-red-400">"fatal"</strong>" vs "<strong class="text-yellow-400">"recoverable"</strong></li>
                        <li>"Rich error context via "<code class="text-blue-400">"ResultExt"</code>" trait"</li>
                    </ul>
                </div>
            </Section>

            // ── Multi-Session Architecture ───────────────────
            <Section title="Multi-Session Architecture">
                <p class="text-slate-400 mb-4">
                    "Flutter Demon supports up to 9 concurrent device sessions, each with its own Flutter process, logs, and state. "
                    <code class="text-blue-400">"SessionManager"</code>" is held on "<code class="text-blue-400">"AppState"</code>" inside "<code class="text-blue-400">"fdemon-app"</code>"."
                </p>
                <DiagramContainer title="Session Hierarchy">
                    <div class="border border-blue-500/30 rounded-lg p-4 bg-blue-950/10">
                        <div class="flex items-center gap-2 mb-1">
                            <Database class="w-4 h-4 text-blue-400" />
                            <span class="font-bold text-blue-400">"SessionManager"</span>
                            <span class="text-xs text-slate-500 font-mono ml-2">"crates/fdemon-app/src/session_manager.rs"</span>
                        </div>
                        <div class="text-xs text-slate-500 mb-4 font-mono">
                            "sessions: HashMap<SessionId, SessionHandle>"<br/>
                            "session_order: Vec<SessionId> \u{00A0}|\u{00A0} selected_index: usize"
                        </div>
                        <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
                            <SessionBox label="SessionHandle 1" />
                            <SessionBox label="SessionHandle 2" />
                            <div class="border border-slate-700/50 rounded-lg p-3 bg-slate-900/30 flex items-center justify-center min-h-[8rem]">
                                <span class="text-xs text-slate-500 italic">"... up to 9 sessions"</span>
                            </div>
                        </div>
                    </div>
                </DiagramContainer>
            </Section>

            // ── Mouse Subsystem ──────────────────────────────
            <Section title="Mouse Subsystem">
                <p class="text-slate-400">
                    "Mouse input flows through the same TEA pipeline as keyboard events. "
                    "The "<code class="text-blue-400">"MouseInput"</code>" type (in "<code class="text-blue-400">"crates/fdemon-app/src/input_mouse.rs"</code>") carries button press, scroll direction, and modifier keys. "
                    "During each render frame, widgets register hit-testable regions via a "<strong class="text-white">"Mouse Region Registry"</strong>" — a per-frame, z-index-aware table of "<code class="text-blue-400">"MouseRegionEntry"</code>" values held on "<code class="text-blue-400">"AppState"</code>" as a "<code class="text-blue-400">"MouseRegionsCell"</code>"."
                </p>
                <p class="text-slate-400">
                    "At frame start, "<code class="text-blue-400">"render::view()"</code>" acquires a "<code class="text-blue-400">"MouseRegionGuard"</code>" (RAII) via "<code class="text-blue-400">"state.mouse_regions.take_guard()"</code>". "
                    "Widgets push click regions through a "<code class="text-blue-400">"MouseRegionsBuilder"</code>" during rendering; the guard returns the populated registry to the cell on drop. "
                    "On "<code class="text-blue-400">"Message::Mouse(Press)"</code>", the handler runs "<code class="text-blue-400">"hit_test(x, y, button)"</code>" against the registry, dispatching the highest-z matching action."
                </p>
                <p class="text-slate-400">
                    "Modal precedence is enforced at the renderer: when a modal is active (confirm dialog, tag filter, new-session dialog), the renderer skips base-UI region registration so only modal regions are clickable. "
                    "See the "<a href="/docs/mouse" class="text-blue-400 hover:underline">"Mouse reference"</a>" for user-facing semantics, per-mode scroll behavior, and platform caveats."
                </p>
            </Section>

            // ── Data Flow: Hot Reload ────────────────────────
            <Section title="Data Flow: Hot Reload">
                <DiagramContainer title="Hot Reload Sequence">
                    <div class="space-y-0">
                        <FlowStep n=1 title="User Trigger" desc="User presses 'r' or FileWatcher detects .dart file change" color=NodeColor::Blue />
                        <FlowStep n=2 title="Message Sent" desc="Message::HotReload dispatched to the update channel" color=NodeColor::Blue />
                        <FlowStep n=3 title="State Transition" desc="handler::update() validates app_id, sets phase to Reloading" color=NodeColor::Orange />
                        <FlowStep n=4 title="Action Dispatched" desc="Returns UpdateAction::SpawnTask(Task::Reload) to event loop" color=NodeColor::Green />
                        <FlowStep n=5 title="JSON-RPC Command" desc="CommandSender sends app.restart via stdin to Flutter process" color=NodeColor::Cyan />
                        <FlowStep n=6 title="Flutter Reload" desc="Flutter process performs hot reload internally" color=NodeColor::Rose />
                        <FlowStep n=7 title="Completion Event" desc="DaemonEvent::Message(AppProgress{finished:true}) received on stdout" color=NodeColor::Cyan />
                        <FlowStep n=8 title="UI Updated" desc="Phase set back to Running, reload count incremented, UI re-rendered" color=NodeColor::Purple />
                    </div>
                </DiagramContainer>
            </Section>

            // ── Data Flow: Log Processing ────────────────────
            <Section title="Data Flow: Log Processing">
                <DiagramContainer title="Log Processing Pipeline">
                    <div class="space-y-0">
                        <FlowStep n=1 title="Process Output" desc="FlutterProcess stdout/stderr reader task receives a line" color=NodeColor::Cyan />
                        <FlowStep n=2 title="Protocol Parse" desc="protocol::parse_daemon_message() converts JSON-RPC to typed event" color=NodeColor::Cyan />
                        <FlowStep n=3 title="Event Dispatch" desc="DaemonEvent::Message(parsed) wrapped as Message::Daemon(event)" color=NodeColor::Blue />
                        <FlowStep n=4 title="State Update" desc="handler::update() processes message, creates LogEntry with level and source" color=NodeColor::Orange />
                        <FlowStep n=5 title="Log Storage" desc="state.add_log(entry) appends to the active session's log buffer" color=NodeColor::Green />
                        <FlowStep n=6 title="UI Render" desc="tui::render() draws the LogView widget with filtering and highlighting" color=NodeColor::Purple />
                    </div>
                </DiagramContainer>
            </Section>

            // ── Key Types ────────────────────────────────────
            <Section title="Key Types">
                <h3 class="text-lg font-bold text-white">"AppState (Model)"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "The complete application state \u{2014} everything needed to render the UI. "
                    "Defined in "<code class="text-blue-400">"crates/fdemon-app/src/state.rs"</code>"."
                </p>
                <CodeBlock code="pub struct AppState {\n    pub ui_mode: UiMode,              // Normal, DevTools, Settings, ...\n    pub session_manager: SessionManager,\n    pub new_session_dialog_state: NewSessionDialogState,\n    pub settings: Settings,\n    pub project_path: PathBuf,\n    pub project_name: Option<String>,\n    // ...\n}" language="rust" />

                <h3 class="text-lg font-bold text-white mt-6">"Message (Events)"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "All possible events that can occur in the application. "
                    "Defined in "<code class="text-blue-400">"crates/fdemon-app/src/message.rs"</code>"."
                </p>
                <CodeBlock code="pub enum Message {\n    // Input\n    Key(KeyEvent),\n    Mouse(MouseInput),\n    Daemon(DaemonEvent),\n    Tick,\n    // Control\n    HotReload, HotRestart, StopApp,\n    // File watcher\n    FilesChanged { count: usize },\n    AutoReloadTriggered,\n    // Session management\n    SelectSessionByIndex(usize),\n    NextSession, PreviousSession,\n    CloseCurrentSession,\n    // Lifecycle\n    Quit,\n    // ...\n}" language="rust" />

                <h3 class="text-lg font-bold text-white mt-6">"handler::update() — The TEA Update Function"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "The update function is a pure function that takes the current state and a message, and returns a new state plus an optional side-effect action. "
                    "Defined in "<code class="text-blue-400">"crates/fdemon-app/src/handler/"</code>"."
                </p>
                <CodeBlock code="pub fn update(state: AppState, message: Message) -> (AppState, Option<UpdateAction>) {\n    // Pure: no side effects. Returns new state + optional action.\n    // ...\n}\n\npub enum UpdateAction {\n    SpawnTask(Task),\n    SpawnSession { device: Device, config: Option<Box<LaunchConfig>> },\n    DiscoverDevices,\n    DiscoverEmulators,\n    LaunchEmulator { emulator_id: String },\n}" language="rust" />
            </Section>

            // ── Project Structure ────────────────────────────
            <Section title="Project Structure">
                <p class="text-slate-400 mb-3">
                    "The repository is a Cargo workspace. All library crates live under "<code class="text-blue-400">"crates/"</code>"; "
                    "the binary entry point is "<code class="text-blue-400">"src/main.rs"</code>"."
                </p>
                <CodeBlock code="flutter-demon/\n\u{251C}\u{2500}\u{2500} Cargo.toml              # Workspace root\n\u{251C}\u{2500}\u{2500} src/\n\u{2502}   \u{251C}\u{2500}\u{2500} main.rs             # Binary entry point, CLI handling\n\u{2502}   \u{2514}\u{2500}\u{2500} headless/           # Headless NDJSON mode\n\u{251C}\u{2500}\u{2500} crates/\n\u{2502}   \u{251C}\u{2500}\u{2500} fdemon-core/        # Domain types (zero deps)\n\u{2502}   \u{2502}   \u{2514}\u{2500}\u{2500} src/\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} types.rs        # LogEntry, LogLevel, AppPhase\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} events.rs       # DaemonEvent + event structs\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} error.rs        # Error enum, Result alias, ResultExt\n\u{2502}   \u{2502}       \u{2514}\u{2500}\u{2500} discovery.rs    # Flutter project detection\n\u{2502}   \u{251C}\u{2500}\u{2500} fdemon-daemon/      # Flutter process I/O (depends: fdemon-core)\n\u{2502}   \u{2502}   \u{2514}\u{2500}\u{2500} src/\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} process.rs      # FlutterProcess lifecycle\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} protocol.rs     # JSON-RPC message parsing\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} commands.rs     # CommandSender, RequestTracker\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} devices.rs      # Device discovery\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} native_logs/    # Android/macOS/iOS log capture\n\u{2502}   \u{2502}       \u{2514}\u{2500}\u{2500} vm_service/     # VM Service WebSocket client\n\u{2502}   \u{251C}\u{2500}\u{2500} fdemon-dap/         # DAP server (depends: fdemon-core)\n\u{2502}   \u{2502}   \u{2514}\u{2500}\u{2500} src/\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} protocol/       # DAP wire types, codec\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} adapter/        # DAP <-> VM Service translation\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} server/         # TCP accept loop, session lifecycle\n\u{2502}   \u{2502}       \u{2514}\u{2500}\u{2500} transport/      # Stdio transport\n\u{2502}   \u{251C}\u{2500}\u{2500} fdemon-app/         # TEA engine (depends: fdemon-core, fdemon-daemon)\n\u{2502}   \u{2502}   \u{2514}\u{2500}\u{2500} src/\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} engine.rs       # Engine - shared orchestration core\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} state.rs        # AppState (the Model)\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} message.rs      # Message enum\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} handler/        # update() function + devtools handlers\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} session/        # Session, SessionHandle, per-session state\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} session_manager.rs\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} config/         # Settings, launch, VSCode config\n\u{2502}   \u{2502}       \u{251C}\u{2500}\u{2500} services/       # FlutterController, LogService, SharedState\n\u{2502}   \u{2502}       \u{2514}\u{2500}\u{2500} new_session_dialog/ # New session dialog state\n\u{2502}   \u{2514}\u{2500}\u{2500} fdemon-tui/         # Terminal UI (depends: fdemon-core, fdemon-app)\n\u{2502}       \u{2514}\u{2500}\u{2500} src/\n\u{2502}           \u{251C}\u{2500}\u{2500} runner.rs       # TUI runner (creates Engine)\n\u{2502}           \u{251C}\u{2500}\u{2500} render/         # State -> UI rendering pipeline\n\u{2502}           \u{251C}\u{2500}\u{2500} widgets/        # Header, tabs, log_view, status_bar\n\u{2502}           \u{2514}\u{2500}\u{2500} widgets/devtools/ # Inspector, Performance, Network, Memory\n\u{2514}\u{2500}\u{2500} tests/                  # Integration tests" language="text" />
            </Section>

            // ── Module Reference ─────────────────────────────
            <Section title="Module Reference">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <ModuleCard title="fdemon-core" desc="Pure domain types \u{2014} zero internal dependencies" accent="text-green-400">
                        <FileEntry name="crates/fdemon-core/src/types.rs" desc="AppPhase, LogEntry, LogLevel, LogSource" />
                        <FileEntry name="crates/fdemon-core/src/events.rs" desc="DaemonEvent \u{2014} events from the Flutter process" />
                        <FileEntry name="crates/fdemon-core/src/error.rs" desc="Custom Error enum, Result<T> alias, ResultExt trait" />
                        <FileEntry name="crates/fdemon-core/src/discovery.rs" desc="Project detection, ProjectType enum" />
                        <FileEntry name="crates/fdemon-core/src/logging.rs" desc="File-based logging via tracing (stdout owned by TUI)" />
                        <FileEntry name="crates/fdemon-core/src/prelude.rs" desc="Re-exports common types and tracing macros" />
                    </ModuleCard>

                    <ModuleCard title="fdemon-daemon" desc="Flutter process management and JSON-RPC" accent="text-cyan-400">
                        <FileEntry name="crates/fdemon-daemon/src/process.rs" desc="FlutterProcess \u{2014} spawns flutter run --machine" />
                        <FileEntry name="crates/fdemon-daemon/src/protocol.rs" desc="parse_daemon_message() from JSON-RPC" />
                        <FileEntry name="crates/fdemon-daemon/src/commands.rs" desc="CommandSender, RequestTracker for request IDs" />
                        <FileEntry name="crates/fdemon-daemon/src/devices.rs" desc="Device discovery and Emulator management" />
                        <FileEntry name="crates/fdemon-daemon/src/native_logs/" desc="Android logcat, macOS log stream, iOS sim/physical" />
                        <FileEntry name="crates/fdemon-daemon/src/vm_service/" desc="VM Service WebSocket client (inspector, perf, network)" />
                    </ModuleCard>

                    <ModuleCard title="fdemon-dap" desc="Debug Adapter Protocol server" accent="text-orange-400">
                        <FileEntry name="crates/fdemon-dap/src/protocol/types.rs" desc="All DAP request, response, and event types" />
                        <FileEntry name="crates/fdemon-dap/src/protocol/codec.rs" desc="Content-Length framing encoder/decoder" />
                        <FileEntry name="crates/fdemon-dap/src/adapter/" desc="DapAdapter, DebugBackend trait, handlers, breakpoints, variables" />
                        <FileEntry name="crates/fdemon-dap/src/server/" desc="DapServer TCP accept loop, client session lifecycle" />
                        <FileEntry name="crates/fdemon-dap/src/transport/stdio.rs" desc="Stdio transport for IDE integration testing" />
                    </ModuleCard>

                    <ModuleCard title="fdemon-app — core" desc="TEA pattern \u{2014} state management and orchestration" accent="text-blue-400">
                        <FileEntry name="crates/fdemon-app/src/engine.rs" desc="Engine \u{2014} shared orchestration core (TUI + headless)" />
                        <FileEntry name="crates/fdemon-app/src/state.rs" desc="AppState \u{2014} the complete application Model" />
                        <FileEntry name="crates/fdemon-app/src/message.rs" desc="Message enum \u{2014} all possible events/actions" />
                        <FileEntry name="crates/fdemon-app/src/signals.rs" desc="Async SIGINT/SIGTERM handler, sends Message::Quit" />
                        <FileEntry name="crates/fdemon-app/src/handler/" desc="update() \u{2014} processes messages, returns (state, action)" />
                        <FileEntry name="crates/fdemon-app/src/session_manager.rs" desc="Manages up to 9 concurrent SessionHandle instances" />
                        <FileEntry name="crates/fdemon-app/src/session/" desc="Session, SessionHandle, PerformanceState, NetworkState, NativeTagState" />
                    </ModuleCard>

                    <ModuleCard title="fdemon-app — config &amp; services" desc="Configuration and service abstractions" accent="text-orange-400">
                        <FileEntry name="crates/fdemon-app/src/config/settings.rs" desc=".fdemon/config.toml loader" />
                        <FileEntry name="crates/fdemon-app/src/config/launch.rs" desc=".fdemon/launch.toml loader" />
                        <FileEntry name="crates/fdemon-app/src/config/vscode.rs" desc=".vscode/launch.json compatibility parser" />
                        <FileEntry name="crates/fdemon-app/src/services/" desc="FlutterController, LogService, SharedState (Arc<RwLock<>>)" />
                        <FileEntry name="crates/fdemon-app/src/watcher.rs" desc="FileWatcher \u{2014} watches lib/ for .dart changes, debounces" />
                    </ModuleCard>

                    <ModuleCard title="fdemon-tui" desc="Presentation layer using ratatui" accent="text-purple-400">
                        <FileEntry name="crates/fdemon-tui/src/runner.rs" desc="TUI runner \u{2014} creates Engine, drives event loop" />
                        <FileEntry name="crates/fdemon-tui/src/render/mod.rs" desc="State \u{2192} UI rendering pipeline" />
                        <FileEntry name="crates/fdemon-tui/src/widgets/header.rs" desc="App header bar with project name and status" />
                        <FileEntry name="crates/fdemon-tui/src/widgets/log_view/" desc="Scrollable log display with syntax highlighting" />
                        <FileEntry name="crates/fdemon-tui/src/widgets/new_session_dialog/" desc="New session creation dialog" />
                        <FileEntry name="crates/fdemon-tui/src/widgets/devtools/" desc="Inspector, Performance, Network, Memory panels" />
                    </ModuleCard>
                </div>
            </Section>

            // ── JSON-RPC Protocol ────────────────────────────
            <Section title="JSON-RPC Protocol">
                <p class="text-slate-400 mb-4">
                    "Flutter\u{2019}s "<code class="text-blue-400">"--machine"</code>" flag outputs JSON-RPC over stdout. "
                    "Messages are wrapped in "<code class="text-blue-400">"[...]"</code>" brackets. "
                    "Parsing is handled by "<code class="text-blue-400">"crates/fdemon-daemon/src/protocol.rs"</code>"."
                </p>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-cyan-400 mb-2">"Events (received)"</h4>
                        <ul class="text-xs text-slate-400 space-y-1 font-mono">
                            <li>"daemon.connected"</li>
                            <li>"app.start"</li>
                            <li>"app.log"</li>
                            <li>"app.progress"</li>
                            <li>"device.added / device.removed"</li>
                        </ul>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-green-400 mb-2">"Commands (sent)"</h4>
                        <ul class="text-xs text-slate-400 space-y-1 font-mono">
                            <li>"app.restart (hot reload/restart)"</li>
                            <li>"app.stop"</li>
                            <li>"daemon.shutdown"</li>
                            <li>"device.getDevices"</li>
                        </ul>
                    </div>
                </div>
            </Section>

            // ── Dependencies ─────────────────────────────────
            <Section title="Key Dependencies">
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Crate"</th>
                                <th class="p-4 font-medium">"Purpose"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <DepRow name="ratatui" purpose="Terminal UI framework" />
                            <DepRow name="crossterm" purpose="Cross-platform terminal manipulation" />
                            <DepRow name="tokio" purpose="Async runtime" />
                            <DepRow name="serde / serde_json" purpose="JSON serialization" />
                            <DepRow name="toml" purpose="TOML config parsing" />
                            <DepRow name="notify" purpose="File system watching" />
                            <DepRow name="tracing" purpose="Structured logging" />
                            <DepRow name="thiserror" purpose="Error derive macros" />
                            <DepRow name="tokio-tungstenite" purpose="VM Service WebSocket client" />
                            <DepRow name="chrono" purpose="Date/time handling" />
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Testing Strategy ─────────────────────────────
            <Section title="Testing Strategy">
                <p class="text-slate-400 mb-4">
                    "Flutter Demon follows Rust\u{2019}s conventional test organization with unit tests alongside source code "
                    "and integration tests in a separate directory."
                </p>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-blue-400 mb-2">"Unit Tests"</h4>
                        <p class="text-xs text-slate-400">
                            "Live alongside source code in each crate. "
                            "Use "<code class="text-blue-400">"#[cfg(test)] mod tests"</code>" inline or separate "
                            <code class="text-blue-400">"tests.rs"</code>" files for large suites."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-green-400 mb-2">"Integration Tests"</h4>
                        <p class="text-xs text-slate-400">
                            "Live in the "<code class="text-blue-400">"tests/"</code>" directory at the workspace root. "
                            "Each file is compiled as a separate crate with access to the public API only."
                        </p>
                    </div>
                </div>

                <h4 class="font-bold text-white text-sm mb-2">"Test Coverage by Crate"</h4>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-3 font-medium">"Crate"</th>
                                <th class="p-3 font-medium hidden md:table-cell">"Approx. tests"</th>
                                <th class="p-3 font-medium">"Coverage notes"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950 text-xs">
                            <TestRow module="fdemon-core" file="357" coverage="Domain types, discovery, stack trace parsing" />
                            <TestRow module="fdemon-daemon" file="527" coverage="JSON-RPC parsing, native log capture, device discovery" />
                            <TestRow module="fdemon-app" file="1511" coverage="State transitions, DevTools handlers, session lifecycle" />
                            <TestRow module="fdemon-tui" file="814" coverage="Widget rendering, DevTools panels, tag filter overlay" />
                            <TestRow module="tests/ (integration)" file="80+" coverage="End-to-end binary tests" />
                        </tbody>
                    </table>
                </div>

                <CodeBlock code="cargo test --workspace      # Run all tests\ncargo test --lib           # Unit tests only\ncargo test -p fdemon-app   # Tests for one crate\ncargo test log_view        # Tests matching pattern\ncargo test -- --nocapture  # With visible output" language="bash" />
            </Section>

            // ── Future Considerations ────────────────────────
            <Section title="Future Considerations">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-blue-400 text-sm">"MCP Server"</h4>
                        <p class="text-xs text-slate-500 mt-1">"Services layer designed for Model Context Protocol integration via EngineEvent subscription"</p>
                    </div>
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-green-400 text-sm">"Plugin System"</h4>
                        <p class="text-xs text-slate-500 mt-1">"Workspace crate separation enables independent extensions and publication"</p>
                    </div>
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-purple-400 text-sm">"Remote Devices"</h4>
                        <p class="text-xs text-slate-500 mt-1">"Device abstraction in fdemon-daemon supports remote device connections"</p>
                    </div>
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-orange-400 text-sm">"DAP Integration"</h4>
                        <p class="text-xs text-slate-500 mt-1">"fdemon-dap provides a full Debug Adapter Protocol server for IDE breakpoint debugging"</p>
                    </div>
                </div>
            </Section>
        </div>
    }
}

// ── Helper Components ────────────────────────────────────────

#[component]
fn Section(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="space-y-4">
            <h2 class="text-xl font-bold text-white flex items-center">
                <div class="w-2 h-6 bg-blue-500 mr-3 rounded-full"></div>
                {title}
            </h2>
            {children()}
        </section>
    }
}

#[component]
fn LayerRow(layer: &'static str, resp: &'static str, deps: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50 transition-colors">
            <td class="p-4 font-mono text-blue-400 font-medium whitespace-nowrap">{layer}</td>
            <td class="p-4 text-slate-300">{resp}</td>
            <td class="p-4 text-slate-500 hidden md:table-cell">{deps}</td>
        </tr>
    }
}

#[component]
fn SessionBox(label: &'static str) -> impl IntoView {
    view! {
        <div class="border border-cyan-500/30 rounded-lg p-3 bg-cyan-950/10">
            <span class="text-xs font-bold text-cyan-400">{label}</span>
            <div class="text-[10px] text-slate-500 mt-1 font-mono">
                "process: Option<FlutterProcess>"<br/>
                "cmd_sender: Option<CommandSender>"
            </div>
            <div class="border border-green-500/30 rounded p-2 mt-2 bg-green-950/10">
                <span class="text-[10px] font-bold text-green-400">"Session"</span>
                <div class="text-[10px] text-slate-500 font-mono">
                    "id, name, phase, device_id"<br/>
                    "logs: Vec<LogEntry>"<br/>
                    "log_view_state, reload_count"
                </div>
            </div>
        </div>
    }
}

#[component]
fn ModuleCard(
    title: &'static str,
    desc: &'static str,
    accent: &'static str,
    children: Children,
) -> impl IntoView {
    let title_cls = format!("font-bold font-mono text-sm {accent}");

    view! {
        <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
            <h4 class=title_cls>{title}</h4>
            <p class="text-xs text-slate-500 mt-1 mb-3">{desc}</p>
            <div class="space-y-1.5">
                {children()}
            </div>
        </div>
    }
}

#[component]
fn FileEntry(name: &'static str, desc: &'static str) -> impl IntoView {
    view! {
        <div class="flex items-baseline gap-2 text-xs">
            <code class="text-blue-400 shrink-0">{name}</code>
            <span class="text-slate-500">{desc}</span>
        </div>
    }
}

#[component]
fn DepRow(name: &'static str, purpose: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50 transition-colors">
            <td class="p-4 font-mono text-blue-400 whitespace-nowrap">{name}</td>
            <td class="p-4 text-slate-300">{purpose}</td>
        </tr>
    }
}

#[component]
fn TestRow(module: &'static str, file: &'static str, coverage: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50 transition-colors">
            <td class="p-3 font-mono text-blue-400 whitespace-nowrap">{module}</td>
            <td class="p-3 text-slate-300 font-mono hidden md:table-cell">{file}</td>
            <td class="p-3 text-slate-500">{coverage}</td>
        </tr>
    }
}
