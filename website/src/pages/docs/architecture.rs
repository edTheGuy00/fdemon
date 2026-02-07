use leptos::prelude::*;

use crate::components::code_block::CodeBlock;
use crate::components::diagrams::*;
use crate::components::icons::*;

#[component]
pub fn Architecture() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-4xl font-bold text-white">"Architecture"</h1>
            <p class="text-lg text-slate-400">
                "Flutter Demon is a terminal-based Flutter development environment built with a "
                "layered architecture separating concerns between domain logic, infrastructure, and presentation. "
                "The application uses The Elm Architecture (TEA) for predictable state management."
            </p>

            // ── System Architecture ─────────────────────────
            <Section title="System Architecture">
                <DiagramContainer title="Architecture Layers">
                    <ArchNode
                        title="Binary (main.rs)"
                        subtitle="CLI parsing, project discovery"
                        color=NodeColor::Slate
                        icon=|| view! { <Terminal class="w-4 h-4" /> }.into_any()
                    />
                    <FlowArrow />
                    <ArchNode
                        title="Application Layer (app/, services/)"
                        subtitle="TEA state management, message handling, service abstractions"
                        color=NodeColor::Blue
                        icon=|| view! { <Layers class="w-4 h-4" /> }.into_any()
                    />
                    <BranchDown3 />
                    <div class="grid grid-cols-1 md:grid-cols-3 gap-3 mx-[12%] md:mx-[16%]">
                        <ArchNode
                            title="Presentation"
                            subtitle="Terminal UI, Ratatui widgets (tui/)"
                            color=NodeColor::Purple
                            icon=|| view! { <Layout class="w-4 h-4" /> }.into_any()
                        />
                        <div>
                            <ArchNode
                                title="Infrastructure"
                                subtitle="Process mgmt, JSON-RPC (daemon/)"
                                color=NodeColor::Cyan
                                icon=|| view! { <Cpu class="w-4 h-4" /> }.into_any()
                            />
                            <FlowArrow />
                            <ArchNode
                                title="Flutter Process"
                                subtitle="flutter run --machine"
                                color=NodeColor::Rose
                                icon=|| view! { <Smartphone class="w-4 h-4" /> }.into_any()
                            />
                        </div>
                        <ArchNode
                            title="Domain"
                            subtitle="Business types, discovery (core/)"
                            color=NodeColor::Green
                            icon=|| view! { <Keyboard class="w-4 h-4" /> }.into_any()
                        />
                    </div>
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
                            <li><strong class="text-white">"Update"</strong>" \u{2014} "<code class="text-blue-400">"handler::update()"</code>" pure function: (State, Msg) \u{2192} (State, Action)"</li>
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
                        title="update(state, message)"
                        subtitle="Pure function \u{2192} (new_state, Option<Action>)"
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
                    "Each layer has clear responsibilities. Dependencies flow downward \u{2014} lower layers never depend on higher ones."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Layer"</th>
                                <th class="p-4 font-medium">"Responsibility"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Dependencies"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <LayerRow layer="Binary" resp="CLI, entry point" deps="All" />
                            <LayerRow layer="App" resp="State, orchestration" deps="Core, Daemon, TUI" />
                            <LayerRow layer="Services" resp="Reusable controllers" deps="Core, Daemon" />
                            <LayerRow layer="TUI" resp="Presentation" deps="Core, App (TEA View)" />
                            <LayerRow layer="Daemon" resp="Flutter process I/O" deps="Core" />
                            <LayerRow layer="Core" resp="Domain types" deps="None" />
                            <LayerRow layer="Common" resp="Utilities" deps="None" />
                        </tbody>
                    </table>
                </div>
                <p class="text-sm text-slate-500 mt-2">
                    "The TUI layer depends on App because of the TEA pattern: "
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
                        <li>"Custom "<code class="text-blue-400">"Error"</code>" enum with domain-specific variants"</li>
                        <li><code class="text-blue-400">"Result<T>"</code>" type alias used throughout the codebase"</li>
                        <li>"Errors are categorized as "<strong class="text-red-400">"fatal"</strong>" vs "<strong class="text-yellow-400">"recoverable"</strong></li>
                        <li>"Rich error context via "<code class="text-blue-400">"ResultExt"</code>" trait"</li>
                    </ul>
                </div>
            </Section>

            // ── Multi-Session Architecture ───────────────────
            <Section title="Multi-Session Architecture">
                <p class="text-slate-400 mb-4">
                    "Flutter Demon supports up to 9 concurrent device sessions, each with its own Flutter process, logs, and state."
                </p>
                <DiagramContainer title="Session Hierarchy">
                    <div class="border border-blue-500/30 rounded-lg p-4 bg-blue-950/10">
                        <div class="flex items-center gap-2 mb-1">
                            <Database class="w-4 h-4 text-blue-400" />
                            <span class="font-bold text-blue-400">"SessionManager"</span>
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
                        <FlowStep n=2 title="Protocol Parse" desc="protocol::DaemonMessage::parse() converts JSON-RPC to typed event" color=NodeColor::Cyan />
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
                <p class="text-slate-400 text-sm mb-2">"The complete application state \u{2014} everything needed to render the UI."</p>
                <CodeBlock code="pub struct AppState {\n    pub ui_mode: UiMode,              // Normal, DeviceSelector, Settings, ...\n    pub session_manager: SessionManager,\n    pub device_selector: DeviceSelectorState,\n    pub settings: Settings,\n    pub project_path: PathBuf,\n    pub project_name: Option<String>,\n    // ...\n}" language="rust" />

                <h3 class="text-lg font-bold text-white mt-6">"Message (Events)"</h3>
                <p class="text-slate-400 text-sm mb-2">"All possible events that can occur in the application."</p>
                <CodeBlock code="pub enum Message {\n    // Input\n    Key(KeyEvent),\n    Daemon(DaemonEvent),\n    Tick,\n    // Control\n    HotReload, HotRestart, StopApp,\n    // File watcher\n    FilesChanged { count: usize },\n    AutoReloadTriggered,\n    // Session management\n    SelectSessionByIndex(usize),\n    NextSession, PreviousSession,\n    CloseCurrentSession,\n    // Lifecycle\n    Quit,\n    // ...\n}" language="rust" />

                <h3 class="text-lg font-bold text-white mt-6">"UpdateResult"</h3>
                <p class="text-slate-400 text-sm mb-2">"Returned by the update function \u{2014} an optional follow-up message and an optional side-effect action."</p>
                <CodeBlock code="pub struct UpdateResult {\n    pub message: Option<Message>,\n    pub action: Option<UpdateAction>,\n}\n\npub enum UpdateAction {\n    SpawnTask(Task),\n    SpawnSession { device: Device, config: Option<Box<LaunchConfig>> },\n    DiscoverDevices,\n    DiscoverEmulators,\n    LaunchEmulator { emulator_id: String },\n}" language="rust" />
            </Section>

            // ── Project Structure ────────────────────────────
            <Section title="Project Structure">
                <CodeBlock code="src/\n\u{251C}\u{2500}\u{2500} main.rs              # Binary entry point, CLI handling\n\u{251C}\u{2500}\u{2500} lib.rs               # Library public API\n\u{251C}\u{2500}\u{2500} common/              # Shared utilities (no dependencies)\n\u{2502}   \u{251C}\u{2500}\u{2500} error.rs         # Error types and Result alias\n\u{2502}   \u{251C}\u{2500}\u{2500} logging.rs       # File-based logging setup\n\u{2502}   \u{2514}\u{2500}\u{2500} prelude.rs       # Common imports\n\u{251C}\u{2500}\u{2500} core/                # Domain types (pure business logic)\n\u{2502}   \u{251C}\u{2500}\u{2500} types.rs         # LogEntry, LogLevel, AppPhase\n\u{2502}   \u{251C}\u{2500}\u{2500} events.rs        # DaemonEvent enum\n\u{2502}   \u{2514}\u{2500}\u{2500} discovery.rs     # Flutter project detection\n\u{251C}\u{2500}\u{2500} config/              # Configuration parsing\n\u{2502}   \u{251C}\u{2500}\u{2500} settings.rs      # .fdemon/config.toml loader\n\u{2502}   \u{251C}\u{2500}\u{2500} launch.rs        # .fdemon/launch.toml loader\n\u{2502}   \u{2514}\u{2500}\u{2500} vscode.rs        # .vscode/launch.json compatibility\n\u{251C}\u{2500}\u{2500} daemon/              # Flutter process management\n\u{2502}   \u{251C}\u{2500}\u{2500} process.rs       # FlutterProcess lifecycle\n\u{2502}   \u{251C}\u{2500}\u{2500} protocol.rs      # JSON-RPC message parsing\n\u{2502}   \u{251C}\u{2500}\u{2500} commands.rs      # Request tracking\n\u{2502}   \u{2514}\u{2500}\u{2500} devices.rs       # Device discovery\n\u{251C}\u{2500}\u{2500} watcher/             # File system watching\n\u{2502}   \u{2514}\u{2500}\u{2500} mod.rs           # Auto-reload on file changes\n\u{251C}\u{2500}\u{2500} services/            # Service abstractions\n\u{2502}   \u{251C}\u{2500}\u{2500} flutter_controller.rs\n\u{2502}   \u{2514}\u{2500}\u{2500} log_service.rs\n\u{251C}\u{2500}\u{2500} app/                 # Application layer (TEA)\n\u{2502}   \u{251C}\u{2500}\u{2500} state.rs         # AppState (the Model)\n\u{2502}   \u{251C}\u{2500}\u{2500} message.rs       # Message enum\n\u{2502}   \u{251C}\u{2500}\u{2500} handler.rs       # update() function\n\u{2502}   \u{2514}\u{2500}\u{2500} session_manager.rs\n\u{2514}\u{2500}\u{2500} tui/                 # Terminal UI (ratatui)\n    \u{251C}\u{2500}\u{2500} render.rs        # State \u{2192} UI rendering\n    \u{251C}\u{2500}\u{2500} event.rs         # Terminal event handling\n    \u{2514}\u{2500}\u{2500} widgets/         # UI components\n        \u{251C}\u{2500}\u{2500} header.rs    # App header bar\n        \u{251C}\u{2500}\u{2500} tabs.rs      # Session tab bar\n        \u{251C}\u{2500}\u{2500} log_view/    # Scrollable log display\n        \u{251C}\u{2500}\u{2500} status_bar.rs\n        \u{2514}\u{2500}\u{2500} device_selector.rs" language="text" />
            </Section>

            // ── Module Reference ─────────────────────────────
            <Section title="Module Reference">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <ModuleCard title="common/" desc="Shared utilities \u{2014} no domain dependencies" accent="text-slate-400">
                        <FileEntry name="error.rs" desc="Custom Error enum, Result<T> alias, ResultExt trait" />
                        <FileEntry name="logging.rs" desc="File-based logging via tracing (stdout owned by TUI)" />
                        <FileEntry name="signals.rs" desc="Async SIGINT/SIGTERM handler, sends Message::Quit" />
                        <FileEntry name="prelude.rs" desc="Re-exports common types and tracing macros" />
                    </ModuleCard>

                    <ModuleCard title="core/" desc="Pure domain types \u{2014} zero external dependencies" accent="text-green-400">
                        <FileEntry name="types.rs" desc="AppPhase, LogEntry, LogLevel, LogSource" />
                        <FileEntry name="events.rs" desc="DaemonEvent \u{2014} events from the Flutter process" />
                        <FileEntry name="discovery.rs" desc="Project detection, ProjectType enum" />
                    </ModuleCard>

                    <ModuleCard title="config/" desc="Configuration parsing from multiple sources" accent="text-orange-400">
                        <FileEntry name="types.rs" desc="LaunchConfig, Settings, FlutterMode types" />
                        <FileEntry name="settings.rs" desc=".fdemon/config.toml loader" />
                        <FileEntry name="launch.rs" desc=".fdemon/launch.toml loader" />
                        <FileEntry name="vscode.rs" desc=".vscode/launch.json compatibility parser" />
                    </ModuleCard>

                    <ModuleCard title="daemon/" desc="Flutter process management and JSON-RPC" accent="text-cyan-400">
                        <FileEntry name="process.rs" desc="FlutterProcess \u{2014} spawns flutter run --machine" />
                        <FileEntry name="protocol.rs" desc="DaemonMessage parsing from JSON-RPC" />
                        <FileEntry name="commands.rs" desc="CommandSender, RequestTracker for request IDs" />
                        <FileEntry name="devices.rs" desc="Device discovery and Emulator management" />
                    </ModuleCard>

                    <ModuleCard title="app/" desc="TEA pattern \u{2014} state management and orchestration" accent="text-blue-400">
                        <FileEntry name="state.rs" desc="AppState \u{2014} the complete application Model" />
                        <FileEntry name="message.rs" desc="Message enum \u{2014} all possible events/actions" />
                        <FileEntry name="handler.rs" desc="update() \u{2014} processes messages, returns state + actions" />
                        <FileEntry name="session_manager.rs" desc="Manages up to 9 concurrent SessionHandle instances" />
                    </ModuleCard>

                    <ModuleCard title="tui/" desc="Presentation layer using ratatui" accent="text-purple-400">
                        <FileEntry name="render.rs" desc="State \u{2192} UI rendering pipeline" />
                        <FileEntry name="event.rs" desc="Terminal event polling (keyboard, resize)" />
                        <FileEntry name="widgets/" desc="Header, SessionTabs, LogView, StatusBar, DeviceSelector" />
                    </ModuleCard>

                    <ModuleCard title="watcher/" desc="File system monitoring for auto-reload" accent="text-yellow-400">
                        <FileEntry name="mod.rs" desc="Watches lib/ for .dart changes, debounces (500ms default)" />
                    </ModuleCard>

                    <ModuleCard title="services/" desc="Abstractions for future MCP server integration" accent="text-rose-400">
                        <FileEntry name="flutter_controller.rs" desc="Trait: reload(), restart(), stop(), is_running()" />
                        <FileEntry name="log_service.rs" desc="Trait: log buffer access and filtering" />
                        <FileEntry name="state_service.rs" desc="SharedState with Arc<RwLock<>>" />
                    </ModuleCard>
                </div>
            </Section>

            // ── JSON-RPC Protocol ────────────────────────────
            <Section title="JSON-RPC Protocol">
                <p class="text-slate-400 mb-4">
                    "Flutter\u{2019}s "<code class="text-blue-400">"--machine"</code>" flag outputs JSON-RPC over stdout. "
                    "Messages are wrapped in "<code class="text-blue-400">"[...]"</code>" brackets."
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
            <Section title="Dependencies">
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
                            <DepRow name="color-eyre" purpose="Enhanced error reporting" />
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
                            "Live in "<code class="text-blue-400">"src/"</code>" alongside the code they test. "
                            "Use "<code class="text-blue-400">"#[cfg(test)] mod tests"</code>" inline or separate "
                            <code class="text-blue-400">"tests.rs"</code>" files for large suites (100+ lines)."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-green-400 mb-2">"Integration Tests"</h4>
                        <p class="text-xs text-slate-400">
                            "Live in the "<code class="text-blue-400">"tests/"</code>" directory at the project root. "
                            "Each file is compiled as a separate crate with access to the public API only."
                        </p>
                    </div>
                </div>

                <h4 class="font-bold text-white text-sm mb-2">"Test Coverage by Module"</h4>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-3 font-medium">"Module"</th>
                                <th class="p-3 font-medium">"Test File"</th>
                                <th class="p-3 font-medium hidden md:table-cell">"Coverage"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950 text-xs">
                            <TestRow module="app/handler" file="tests.rs" coverage="Message handling, state transitions" />
                            <TestRow module="app/session" file="tests.rs" coverage="Session lifecycle, log management" />
                            <TestRow module="core/discovery" file="inline" coverage="Project detection logic" />
                            <TestRow module="daemon/protocol" file="inline" coverage="JSON-RPC parsing" />
                            <TestRow module="tui/render" file="render/tests.rs" coverage="Full-screen snapshots, UI transitions" />
                            <TestRow module="tui/widgets/log_view" file="tests.rs" coverage="Widget rendering, scrolling" />
                        </tbody>
                    </table>
                </div>

                <CodeBlock code="cargo test              # Run all tests\ncargo test --lib        # Unit tests only\ncargo test --test '*'   # Integration tests only\ncargo test log_view     # Tests matching pattern\ncargo test -- --nocapture  # With visible output" language="bash" />
            </Section>

            // ── Future Considerations ────────────────────────
            <Section title="Future Considerations">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-blue-400 text-sm">"MCP Server"</h4>
                        <p class="text-xs text-slate-500 mt-1">"Services layer designed for Model Context Protocol integration"</p>
                    </div>
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-green-400 text-sm">"Plugin System"</h4>
                        <p class="text-xs text-slate-500 mt-1">"Core/service separation enables plugin extensions"</p>
                    </div>
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-purple-400 text-sm">"Remote Devices"</h4>
                        <p class="text-xs text-slate-500 mt-1">"Device abstraction supports remote device connections"</p>
                    </div>
                    <div class="p-3 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-orange-400 text-sm">"Themes"</h4>
                        <p class="text-xs text-slate-500 mt-1">"UI settings include theme configuration placeholder"</p>
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
            <td class="p-3 text-slate-300 font-mono">{file}</td>
            <td class="p-3 text-slate-500 hidden md:table-cell">{coverage}</td>
        </tr>
    }
}
