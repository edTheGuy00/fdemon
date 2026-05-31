use leptos::prelude::*;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Configuration() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Configuration"</h1>
            <p class="text-slate-400">
                "Flutter Demon uses a hierarchical configuration system. All files are optional \u{2014} it works out-of-the-box with sensible defaults."
            </p>

            // ── Configuration Files ──────────────────────────────────
            <Section title="Configuration Files">
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"File"</th>
                                <th class="p-4 font-medium">"Purpose"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Git?"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">".fdemon/config.toml"</td>
                                <td class="p-4 text-white">"Project settings (shared with team)"</td>
                                <td class="p-4 text-green-400 hidden md:table-cell">"Yes"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">".fdemon/launch.toml"</td>
                                <td class="p-4 text-white">"Launch configurations"</td>
                                <td class="p-4 text-green-400 hidden md:table-cell">"Yes"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">".fdemon/settings.local.toml"</td>
                                <td class="p-4 text-white">"User preferences (local overrides)"</td>
                                <td class="p-4 text-red-400 hidden md:table-cell">"No (gitignored)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">".vscode/launch.json"</td>
                                <td class="p-4 text-white">"VSCode launch configs (read-only)"</td>
                                <td class="p-4 text-green-400 hidden md:table-cell">"Yes"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Behavior Settings ────────────────────────────────────
            <Section title="Behavior Settings">
                <CodeBlock language="toml" code="[behavior]\nconfirm_quit = true              # Show confirmation when quitting with active sessions\nauto_launch = false              # Set true to auto-launch on the device cached in settings.local.toml\nversion_check = true             # Check GitHub for a newer release on startup\nversion_check_timeout_secs = 3  # HTTP timeout for the version check (0 = disable)" />
                <SettingsTable entries=vec![
                    ("confirm_quit", "boolean", "true", "If true, shows confirmation dialog when quitting with running apps"),
                    ("auto_launch", "boolean", "false", "When true, fdemon auto-launches the cached last_device on startup if no launch.toml config has auto_start = true. When false (default), the cache only pre-selects a default in the New Session dialog. Has no effect in headless mode."),
                    ("version_check", "boolean", "true", "When true (default), fdemon checks GitHub for a newer release on startup and shows a one-line banner if one is available. Set to false to opt out."),
                    ("version_check_timeout_secs", "integer", "3", "Total HTTP timeout in seconds for the GitHub release version check. Increase on slow connections; set to 0 to disable the check entirely."),
                ] />
                <div class="bg-amber-900/20 border border-amber-800 p-4 rounded-lg text-amber-200 text-sm">
                    <p class="font-medium mb-1">"Deprecated: "<code class="text-amber-300">"[behavior] auto_start"</code></p>
                    <p>
                        "Removed in v0.5.0. Use per-config "<code class="text-amber-300">"auto_start = true"</code>
                        " in "<code class="text-amber-300">".fdemon/launch.toml"</code>" instead. Existing configs that still set the flag load without error \u{2014} Flutter Demon logs a one-time deprecation warning and ignores the value. "
                        <code class="text-amber-300">"[behavior] auto_launch"</code>" is a "<em>"new"</em>" field, not a revival of "<code class="text-amber-300">"auto_start"</code>"."
                    </p>
                </div>
            </Section>

            // ── Watcher Settings ─────────────────────────────────────
            <Section title="Watcher Settings">
                <p class="text-slate-400">"Configure the file watcher for automatic hot reload."</p>
                <CodeBlock language="toml" code="[watcher]\npaths = [\"lib\"]              # Directories to watch\ndebounce_ms = 500            # Delay before triggering reload\nauto_reload = true           # Enable automatic hot reload\nextensions = [\"dart\"]        # File extensions to monitor" />
                <SettingsTable entries=vec![
                    ("paths", "array<string>", r#"["lib"]"#, "Directories to watch for changes, relative to project root"),
                    ("debounce_ms", "integer", "500", "Debounce delay in ms. Prevents reload spam on rapid changes"),
                    ("auto_reload", "boolean", "true", "Automatically trigger hot reload when watched files change"),
                    ("extensions", "array<string>", r#"["dart"]"#, "File extensions to monitor"),
                ] />
            </Section>

            // ── UI Settings ──────────────────────────────────────────
            <Section title="UI Settings">
                <CodeBlock language="toml" code="[ui]\nlog_buffer_size = 10000         # Max log entries in memory\nshow_timestamps = true          # Display timestamps\ncompact_logs = false            # Collapse similar entries\ntheme = \"default\"               # Color theme\nstack_trace_collapsed = true    # Start stack traces collapsed\nstack_trace_max_frames = 3      # Frames shown when collapsed\nicons = \"nerd_fonts\"            # Icon style: \"nerd_fonts\" (default) or \"unicode\"\nenable_mouse = true             # Capture mouse events for clickable UI; restart required" />
                <SettingsTable entries=vec![
                    ("log_buffer_size", "integer", "10000", "Max log entries to retain. Older entries are discarded"),
                    ("show_timestamps", "boolean", "true", "Display timestamps for each log entry"),
                    ("compact_logs", "boolean", "false", "Collapse similar consecutive log entries"),
                    ("theme", "string", "\"default\"", "Color theme name"),
                    ("stack_trace_collapsed", "boolean", "true", "Stack traces start collapsed by default"),
                    ("stack_trace_max_frames", "integer", "3", "Frames to show when collapsed. Press Enter to expand"),
                    ("icons", "string", "\"nerd_fonts\"", "Icon rendering mode. \"nerd_fonts\" (default) uses Nerd Font glyphs — requires a Nerd Font installed in the terminal. \"unicode\" uses safe Unicode characters that work in all terminals. Can also be overridden with FDEMON_ICONS env var."),
                    ("enable_mouse", "boolean", "true", "Enables terminal mouse capture for clickable UI surfaces. When false, fdemon does not emit mouse-capture escape sequences, leaving native terminal behavior (text selection, wheel scrollback) intact. Restart required after changing."),
                ] />
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm">
                    <p class="font-medium mb-1">"Mouse capture ("<code class="text-blue-300">"enable_mouse"</code>")"</p>
                    <p>
                        "Most modern terminals pass "<code class="text-blue-300">"Shift+drag"</code>" through to native text selection even when capture is on, so the default "<code class="text-blue-300">"true"</code>" works for most users. "
                        "Disable if your terminal does not support "<code class="text-blue-300">"Shift+drag"</code>" for native selection, or if you prefer native wheel scrollback over fdemon\u{2019}s in-app scrolling. "
                        "See the "<a href="/docs/mouse" class="text-blue-400 hover:underline">"Mouse reference"</a>" for per-mode wheel behavior, modifier keys, and platform caveats."
                    </p>
                </div>
            </Section>

            // ── DevTools Settings ────────────────────────────────────
            <Section title="DevTools Settings">
                <CodeBlock language="toml" code="[devtools]
auto_open = false                          # Auto-open DevTools on app start
browser = \"\"                               # Browser command (empty = system default)
default_panel = \"inspector\"               # Default panel: \"inspector\", \"performance\", \"network\", \"memory\"
performance_refresh_ms = 2000              # Memory polling interval (ms, min 500)
memory_history_size = 60                   # Memory snapshots to retain
tree_max_depth = 0                         # Widget tree max depth (0 = unlimited)
inspector_fetch_timeout_secs = 60          # Widget tree fetch timeout with retries (min 5s)
auto_repaint_rainbow = false               # Auto-enable repaint rainbow on connect
auto_performance_overlay = false           # Auto-enable performance overlay on connect
allocation_profile_interval_ms = 5000      # Class allocation fetch interval (min 1000ms)
max_network_entries = 500                  # Max HTTP entries per session (FIFO eviction)
network_auto_record = true                 # Auto-start recording when entering Network tab
network_poll_interval_ms = 1000            # HTTP profile poll interval (min 500ms)
inspector_readiness_poll_attempts = 2      # isWidgetTreeReady poll attempts before proceeding
inspector_readiness_poll_interval_ms = 250 # Sleep between poll attempts (ms)
inspector_readiness_poll_call_timeout_ms = 1000 # Per-call timeout for isWidgetTreeReady RPC (ms)
hide_implementation_widgets = true         # Collapse single-child wrapper chains in Inspector (toggle: Shift+H)
auto_enable_rebuild_tracking = false       # Auto-enable widget rebuild tracking on VM connect
rebuild_stats_frame_window = 30            # Frames to keep in rebuild stats ring buffer
timeline_event_buffer_size = 10000         # Max timeline events kept in memory

[devtools.logging]
hybrid_enabled = true          # Enable hybrid logging (VM Service + daemon)
prefer_vm_level = true         # Prefer VM Service log level when available
show_source_indicator = false  # Show [VM]/[daemon] tags on log entries
dedupe_threshold_ms = 100      # Dedup threshold for matching logs (ms)" />
                <SettingsTable entries=vec![
                    ("auto_open", "boolean", "false", "Automatically open DevTools in a browser when app starts"),
                    ("browser", "string", "\"\"", "Browser command (e.g. \"chrome\", \"firefox\"). Empty = system default"),
                    ("default_panel", "string", "\"inspector\"", "Default panel when entering DevTools mode. Options: \"inspector\", \"performance\", \"network\", \"memory\""),
                    ("performance_refresh_ms", "integer", "2000", "Memory/performance data polling interval in milliseconds. Minimum 500"),
                    ("memory_history_size", "integer", "60", "Number of memory snapshots to retain in the ring buffer"),
                    ("tree_max_depth", "integer", "0", "Max depth when fetching widget tree. 0 = unlimited"),
                    ("inspector_fetch_timeout_secs", "integer", "60", "Widget tree fetch timeout in seconds (with readiness polling and retries). Minimum effective value is 5 seconds."),
                    ("auto_repaint_rainbow", "boolean", "false", "Automatically enable repaint rainbow overlay when VM connects"),
                    ("auto_performance_overlay", "boolean", "false", "Automatically enable performance overlay when VM connects"),
                    ("allocation_profile_interval_ms", "integer", "5000", "How often getAllocationProfile is called to capture per-class heap statistics. Clamped to minimum 1000ms."),
                    ("max_network_entries", "integer", "500", "Maximum number of HTTP network entries to keep per session. Oldest entries are evicted (FIFO) when the limit is reached."),
                    ("network_auto_record", "boolean", "true", "Automatically start network recording when entering the Network tab."),
                    ("network_poll_interval_ms", "integer", "1000", "How often getHttpProfile is called when network recording is active. Clamped to minimum 500ms."),
                    ("inspector_readiness_poll_attempts", "integer", "2", "Number of isWidgetTreeReady poll attempts before proceeding with the fetch anyway."),
                    ("inspector_readiness_poll_interval_ms", "integer", "250", "Milliseconds to sleep between consecutive isWidgetTreeReady poll calls."),
                    ("inspector_readiness_poll_call_timeout_ms", "integer", "1000", "Per-call timeout in milliseconds for each isWidgetTreeReady RPC. A timed-out call is treated as not ready."),
                    ("hide_implementation_widgets", "boolean", "true", "Collapse long single-child chains of non-local wrapper widgets in the Inspector tree. Toggle at runtime with Shift+H."),
                    ("auto_enable_rebuild_tracking", "boolean", "false", "Automatically enable widget rebuild tracking on VM Service connect. Adds overhead in dev builds; off by default."),
                    ("rebuild_stats_frame_window", "integer", "30", "Number of recent frames to keep in the rebuild stats ring buffer (~0.5s at 60 FPS)."),
                    ("timeline_event_buffer_size", "integer", "10000", "Max timeline events kept in memory. The timeline polling task evicts oldest events when the buffer is full."),
                ] />

                <h3 class="text-lg font-bold text-white mt-6">"Logging Settings"</h3>
                <p class="text-slate-400 text-sm">
                    "Configure hybrid logging behavior when both VM Service and daemon log sources are available."
                </p>
                <SettingsTable entries=vec![
                    ("hybrid_enabled", "boolean", "true", "Enable hybrid logging. When true, merges VM Service logs with daemon stdout logs"),
                    ("prefer_vm_level", "boolean", "true", "Use VM Service log level (accurate) over content-based level detection"),
                    ("show_source_indicator", "boolean", "false", "Show [VM] or [daemon] tags next to each log entry to indicate its source"),
                    ("dedupe_threshold_ms", "integer", "100", "Logs from both sources within this window (ms) with matching content are deduplicated"),
                ] />

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm">
                    <p class="font-medium mb-2">"Notes"</p>
                    <ul class="list-disc list-inside space-y-1">
                        <li><code class="text-blue-400">"performance_refresh_ms"</code>" controls how often memory usage is polled. Lower values give more granular data but increase VM Service traffic. Frame timing and GC events are streamed in real-time regardless of this setting."</li>
                        <li><code class="text-blue-400">"tree_max_depth"</code>" can improve performance for apps with very deep widget trees. A value of 0 (default) fetches the entire tree."</li>
                        <li>"Auto-overlay settings ("<code class="text-blue-400">"auto_repaint_rainbow"</code>", "<code class="text-blue-400">"auto_performance_overlay"</code>") activate overlays on the device/emulator screen as soon as the VM Service connects."</li>
                    </ul>
                </div>
            </Section>

            // ── Editor Settings ──────────────────────────────────────
            <Section title="Editor Settings">
                <p class="text-slate-400">"Configure editor integration for opening files from stack traces and link mode."</p>
                <CodeBlock language="toml" code="[editor]\ncommand = \"\"                        # Auto-detect from environment\nopen_pattern = \"$EDITOR $FILE:$LINE\"  # Pattern for opening files" />

                <h3 class="text-lg font-bold text-white mt-6">"Auto-Detection Priority"</h3>
                <ol class="list-decimal list-inside text-slate-400 space-y-1 ml-2">
                    <li><strong class="text-white">"Parent IDE"</strong>" \u{2014} Detects if running inside VS Code, Cursor, Zed, IntelliJ, or Neovim terminal"</li>
                    <li><code class="text-blue-400">"$VISUAL"</code>" environment variable"</li>
                    <li><code class="text-blue-400">"$EDITOR"</code>" environment variable"</li>
                    <li><strong class="text-white">"PATH search"</strong>" \u{2014} Checks for code, cursor, zed, nvim, vim, emacs, subl, idea"</li>
                </ol>

                <h3 class="text-lg font-bold text-white mt-6">"Supported Editors"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Editor"</th>
                                <th class="p-4 font-medium">"Command"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Default Pattern"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <EditorRow editor="VS Code" command="code" pattern="code --reuse-window --goto $FILE:$LINE:$COLUMN" />
                            <EditorRow editor="Cursor" command="cursor" pattern="cursor --reuse-window --goto $FILE:$LINE:$COLUMN" />
                            <EditorRow editor="Zed" command="zed" pattern="zed $FILE:$LINE" />
                            <EditorRow editor="Neovim" command="nvim" pattern="nvim +$LINE $FILE" />
                            <EditorRow editor="Vim" command="vim" pattern="vim +$LINE $FILE" />
                            <EditorRow editor="Emacs" command="emacs" pattern="emacs +$LINE:$COLUMN $FILE" />
                            <EditorRow editor="Sublime Text" command="subl" pattern="subl $FILE:$LINE:$COLUMN" />
                            <EditorRow editor="IntelliJ IDEA" command="idea" pattern="idea --line $LINE $FILE" />
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Launch Configuration ─────────────────────────────────
            <Section title="Launch Configuration">
                <p class="text-slate-400">
                    "Define how to run your Flutter app with specific settings using "
                    <code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/launch.toml"</code>
                    "."
                </p>
                <CodeBlock language="toml" code="[[configurations]]\nname = \"Development\"\ndevice = \"auto\"              # \"auto\" or specific device ID\nmode = \"debug\"               # debug, profile, or release\nflavor = \"development\"       # optional\nentry_point = \"lib/main_dev.dart\"  # optional\nauto_start = true            # optional, default false\n\n[configurations.dart_defines]\nAPI_URL = \"https://dev.api.com\"\nDEBUG = \"true\"" />

                <h3 class="text-lg font-bold text-white mt-6">"Configuration Properties"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Property"</th>
                                <th class="p-4 font-medium">"Type"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Description"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <PropRow prop="name" typ="string" desc="Display name (required)" />
                            <PropRow prop="device" typ="string" desc="Target device: \"auto\", platform prefix, partial or exact ID" />
                            <PropRow prop="mode" typ="string" desc="Build mode: \"debug\", \"profile\", or \"release\"" />
                            <PropRow prop="flavor" typ="string" desc="Build flavor (e.g. \"development\", \"production\")" />
                            <PropRow prop="entry_point" typ="string" desc="Entry point file path (default: lib/main.dart)" />
                            <PropRow prop="dart_defines" typ="table" desc="Key-value pairs passed as --dart-define flags" />
                            <PropRow prop="extra_args" typ="array" desc="Additional arguments passed to flutter run" />
                            <PropRow prop="auto_start" typ="boolean" desc="Start automatically when Flutter Demon launches" />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Flutter Modes"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Mode"</th>
                                <th class="p-4 font-medium">"Description"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Use Case"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"debug"</td>
                                <td class="p-4 text-white">"Full debugging, assertions enabled"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Development"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-yellow-400">"profile"</td>
                                <td class="p-4 text-white">"Some optimizations, profiling enabled"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Performance testing"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-green-400">"release"</td>
                                <td class="p-4 text-white">"Full optimizations, no debugging"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Production builds"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Device Selection"</h3>
                <p class="text-slate-400 mb-2">"The "<code class="text-blue-400">"device"</code>" property accepts:"</p>
                <CodeBlock language="toml" code="device = \"auto\"              # First available\ndevice = \"ios\"               # Any iOS device/simulator\ndevice = \"android\"           # Any Android device/emulator\ndevice = \"iphone\"            # Matches \"iPhone 15 Pro\"\ndevice = \"chrome\"            # Web on Chrome" />

                <h3 class="text-lg font-bold text-white mt-6">"Auto-Start Behavior"</h3>
                <p class="text-slate-400">
                    "Flutter Demon auto-launches a session at startup when "
                    <strong class="text-white">"either"</strong>
                    ":"
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 text-sm mt-2">
                    <li>
                        "any configuration in "
                        <code class="text-blue-400 bg-slate-900 px-1 rounded">"launch.toml"</code>
                        " sets "<code class="text-blue-400">"auto_start = true"</code>" (per-config explicit intent), "
                        <strong class="text-white">"or"</strong>
                    </li>
                    <li>
                        <code class="text-blue-400 bg-slate-900 px-1 rounded">"[behavior] auto_launch = true"</code>
                        " is set in "<code class="text-blue-400">"config.toml"</code>" "
                        <strong class="text-white">"and"</strong>
                        " a valid "<code class="text-blue-400">"last_device"</code>" is cached in "
                        <code class="text-blue-400">"settings.local.toml"</code>" (cache-based opt-in)."
                    </li>
                </ul>
                <p class="text-slate-400 mt-2 text-sm">
                    "Otherwise, the New Session dialog opens. The cached "
                    <code class="text-blue-400">"last_device"</code>" (if any) pre-selects in the dialog but does not trigger a launch."
                </p>

                <h4 class="font-bold text-white mt-4">"Selection Priority"</h4>
                <p class="text-slate-400 text-sm">"First matching tier wins:"</p>
                <ol class="list-decimal list-inside text-slate-400 space-y-2 ml-2 text-sm">
                    <li>
                        <strong class="text-white">"Explicit intent"</strong>
                        " \u{2014} first launch config with "<code class="text-blue-400">"auto_start = true"</code>
                        ". Always beats the cache. If its "<code class="text-blue-400">"device"</code>
                        " is not connected, Flutter Demon uses the first available device (still Tier 1) and writes a warning to the fdemon log file."
                    </li>
                    <li>
                        <strong class="text-white">"Cache opt-in"</strong>
                        " \u{2014} reachable only when "<code class="text-blue-400">"[behavior] auto_launch = true"</code>
                        " and no config has "<code class="text-blue-400">"auto_start = true"</code>
                        ". If "<code class="text-blue-400">"settings.local.toml"</code>" holds "
                        <code class="text-blue-400">"last_device"</code>" + "
                        <code class="text-blue-400">"last_config"</code>
                        " and the device is still connected, that selection is used. Falls through to Tier 3 if the saved device has been disconnected, writing a warning to the fdemon log file."
                    </li>
                    <li>
                        <strong class="text-white">"First available"</strong>
                        " \u{2014} reachable only when "<code class="text-blue-400">"[behavior] auto_launch = true"</code>
                        " and the cache is stale or missing. First config in "<code class="text-blue-400">"launch.toml"</code>
                        " (or "<code class="text-blue-400">"launch.json"</code>") + first discovered device."
                    </li>
                    <li>
                        <strong class="text-white">"Bare "</strong>
                        <code class="text-blue-400">"flutter run"</code>
                        " \u{2014} if no configs exist at all."
                    </li>
                </ol>

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <p class="font-medium mb-1">"Headless mode"</p>
                    <p>
                        "In headless mode ("<code class="text-blue-400">"fdemon --headless"</code>"), cache-based auto-launch (Tier 2) is "
                        <strong>"always disabled"</strong>" \u{2014} it is designed for interactive terminal sessions only. Headless honoring of "
                        <code class="text-blue-400">"auto_start = true"</code>" (Tier 1) is fully supported."
                    </p>
                </div>

                <h4 class="font-bold text-white mt-4">"Cache Updates"</h4>
                <p class="text-slate-400 text-sm">
                    <code class="text-blue-400">"last_device"</code>" and "
                    <code class="text-blue-400">"last_config"</code>
                    " are written to "<code class="text-blue-400">"settings.local.toml"</code>
                    " whenever a session starts successfully \u{2014} from both auto-launch and manual selections in the New Session dialog. "
                    "The cache pre-selects the default device in the New Session dialog so your last choice is remembered, but it only triggers an auto-launch when "
                    <code class="text-blue-400">"[behavior] auto_launch = true"</code>"."
                </p>

                <h3 class="text-lg font-bold text-white mt-6">"Dart Defines"</h3>
                <p class="text-slate-400 mb-2">"Pass compile-time constants to your Dart code:"</p>
                <CodeBlock language="toml" code="[configurations.dart_defines]\nAPI_URL = \"https://api.example.com\"\nFEATURE_FLAG_X = \"true\"\nDEBUG_MODE = \"false\"" />
                <p class="text-slate-400 mt-2">"Access in Dart via "<code class="text-blue-400">"String.fromEnvironment('API_URL')"</code>"."</p>
            </Section>

            // ── VSCode Integration ───────────────────────────────────
            <Section title="VSCode Integration">
                <p class="text-slate-400">
                    "Flutter Demon automatically imports "
                    <code class="text-blue-400 bg-slate-900 px-1 rounded">".vscode/launch.json"</code>
                    " configurations. Only entries with "<code class="text-blue-400">"\"type\": \"dart\""</code>" are imported. These are read-only in Flutter Demon."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"Property Mapping"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"VSCode"</th>
                                <th class="p-4 font-medium">"Flutter Demon"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"name"</td>
                                <td class="p-4 font-mono text-green-400">"name"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"program"</td>
                                <td class="p-4 font-mono text-green-400">"entry_point"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"deviceId"</td>
                                <td class="p-4 font-mono text-green-400">"device"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"flutterMode"</td>
                                <td class="p-4 font-mono text-green-400">"mode"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"toolArgs"</td>
                                <td class="p-4 text-slate-400">"Parsed into dart_defines, flavor, extra_args"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Note:"</strong>
                    " VSCode-imported configurations never auto-start, and JSONC (JSON with Comments) is fully supported."
                </div>
            </Section>

            // ── Settings Panel ───────────────────────────────────────
            <Section title="Settings Panel">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400 bg-slate-900 px-1 rounded">","</code>" (comma) from normal mode to open the built-in settings panel."
                </p>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"1. Project Settings"</h4>
                        <p class="text-sm text-slate-400">"Edit .fdemon/config.toml (shared with team)"</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"2. User Preferences"</h4>
                        <p class="text-sm text-slate-400">"Edit .fdemon/settings.local.toml (personal)"</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"3. Launch Config"</h4>
                        <p class="text-sm text-slate-400">"Manage .fdemon/launch.toml configurations"</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"4. VSCode Config"</h4>
                        <p class="text-sm text-slate-400">"View .vscode/launch.json (read-only)"</p>
                    </div>
                </div>
                <p class="text-slate-400 mt-4">
                    "Use "<code class="text-blue-400">"Tab"</code>"/"<code class="text-blue-400">"Shift+Tab"</code>
                    " to cycle tabs, "<code class="text-blue-400">"1-4"</code>" to jump, and "
                    <code class="text-blue-400">"Ctrl+S"</code>" to save. See the "
                    <a href="/docs/keybindings" class="text-blue-400 hover:underline">"Keybindings"</a>
                    " page for full controls."
                </p>
            </Section>

            // ── Native Logs Settings ─────────────────────────────────
            <Section title="Native Logs Settings">
                <p class="text-slate-400">
                    "Configure parallel native log capture (Android logcat, macOS log stream, iOS simulator/device). "
                    "See the "<a href="/docs/native-logs" class="text-blue-400 hover:underline">"Native Logs"</a>" page for full details."
                </p>
                <CodeBlock language="toml" code="[native_logs]
enabled = true                  # Master toggle for native log capture
exclude_tags = [\"flutter\"]     # Tags to exclude (default: flutter, to avoid duplication)
include_tags = []               # If non-empty, ONLY show these tags (overrides exclude_tags)
min_level = \"info\"             # Minimum priority: \"verbose\", \"debug\", \"info\", \"warning\", \"error\"

# Per-tag level overrides
[native_logs.tags.OkHttp]
min_level = \"warning\"          # Show only warnings and above for OkHttp

[native_logs.tags.\"com.example.myplugin\"]
min_level = \"verbose\"          # Verbose logging for a specific plugin

# Custom log source process
[[native_logs.custom_sources]]
name = \"backend\"               # Display name / tag in log view
command = \"docker\"
args = [\"logs\", \"-f\", \"my-backend\"]
format = \"raw\"                 # raw, json, logcat_threadtime, syslog" />
                <SettingsTable entries=vec![
                    ("enabled", "boolean", "true", "Master toggle for native log capture. When false, no native log processes are spawned."),
                    ("exclude_tags", "array<string>", r#"["flutter"]"#, "Tags to exclude from native log output. Default excludes Flutter's own tag to avoid duplicating logs already captured via --machine."),
                    ("include_tags", "array<string>", "[]", "If non-empty, operates in whitelist mode: only logs from these tags are shown and exclude_tags is ignored."),
                    ("min_level", "string", "\"info\"", "Minimum native log priority level. Logs below this level are discarded. Options: \"verbose\", \"debug\", \"info\", \"warning\", \"error\"."),
                ] />
                <h3 class="text-lg font-bold text-white mt-6">"Per-Tag Level Overrides"</h3>
                <p class="text-slate-400 text-sm">
                    "Use "<code class="text-blue-400">"[native_logs.tags.TAG_NAME]"</code>" to set a minimum log level for a specific tag, "
                    "overriding the global "<code class="text-blue-400">"min_level"</code>". "
                    "For dotted tag names, quote the key: "<code class="text-blue-400">"[native_logs.tags.\"com.example.plugin\"]"</code>"."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Field"</th>
                                <th class="p-4 font-medium">"Type"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Description"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"min_level"</td>
                                <td class="p-4 font-mono text-slate-300">"string"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Minimum log level for this tag. Overrides the global min_level for matching log entries."</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <h3 class="text-lg font-bold text-white mt-6">"Custom Log Sources"</h3>
                <p class="text-slate-400 text-sm">
                    "Use "<code class="text-blue-400">"[[native_logs.custom_sources]]"</code>" (array of tables) to capture output from any external command alongside native platform logs."
                </p>
                <SettingsTable entries=vec![
                    ("name", "string", "(required)", "Display name — becomes the tag in the log view and tag filter overlay. Must be unique (case-insensitive)."),
                    ("command", "string", "(required)", "Path to the command to execute (e.g. \"adb\", \"/usr/local/bin/my-tool\")."),
                    ("args", "array<string>", "[]", "Command arguments."),
                    ("format", "string", "\"raw\"", "Output format parser. Options: \"raw\", \"json\", \"logcat_threadtime\", \"syslog\" (macOS only)."),
                    ("working_dir", "string", "project root", "Working directory for the command. Defaults to the Flutter project root."),
                    ("start_before_app", "boolean", "false", "When true, the source is spawned during the pre-app phase. Its readiness check (if any) must pass before Flutter launches."),
                    ("shared", "boolean", "false", "When true, the source is spawned once and its logs are broadcast to all sessions. When false, a new process is started per session."),
                ] />
            </Section>

            // ── DAP Server Settings ──────────────────────────────────
            <Section title="DAP Server Settings">
                <p class="text-slate-400">
                    "Configure the embedded Debug Adapter Protocol (DAP) server for IDE debugger integration."
                </p>
                <CodeBlock language="toml" code="[dap]
enabled = false                # Always enable DAP server on startup (or use --dap CLI flag)
auto_start_in_ide = true       # Auto-start when running inside a detected IDE terminal
port = 0                       # TCP port (0 = auto-assign an available port)
bind_address = \"127.0.0.1\"   # Bind address for the DAP server
suppress_reload_on_pause = true  # Suppress hot reload while debugger is paused at a breakpoint
auto_configure_ide = true      # Auto-generate IDE DAP config (launch.json / languages.toml) on bind" />
                <SettingsTable entries=vec![
                    ("enabled", "boolean", "false", "Always enable DAP server on startup. Overrides auto-detection. Can also be set via --dap CLI flag."),
                    ("auto_start_in_ide", "boolean", "true", "Auto-start DAP server when running inside a detected IDE terminal (VS Code, Neovim, Helix, Zed, Emacs). Has no effect when enabled = true."),
                    ("port", "integer", "0", "TCP port for DAP connections. 0 = auto-assign an available port. Use a fixed port for stable IDE configs across restarts."),
                    ("bind_address", "string", "\"127.0.0.1\"", "Bind address for the DAP server. Restrict to loopback (default) for local development."),
                    ("suppress_reload_on_pause", "boolean", "true", "Suppress automatic hot reload while the debugger is paused at a breakpoint, preventing the session from being disrupted."),
                    ("auto_configure_ide", "boolean", "true", "Automatically generate IDE DAP configuration when the server starts (e.g. .vscode/launch.json, .helix/languages.toml). Set to false to manage configs manually."),
                ] />
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <p class="font-medium mb-1">"Detected IDEs"</p>
                    <p>
                        "When "<code class="text-blue-300">"auto_start_in_ide = true"</code>", fdemon detects the parent IDE via environment variables: "
                        "VS Code / Cursor (<code class=\"text-blue-300\">TERM_PROGRAM</code>), "
                        "Zed (<code class=\"text-blue-300\">ZED_TERM</code>), "
                        "Neovim (<code class=\"text-blue-300\">NVIM</code>), "
                        "Emacs (<code class=\"text-blue-300\">INSIDE_EMACS</code>), "
                        "Helix (<code class=\"text-blue-300\">HELIX_RUNTIME</code>), "
                        "JetBrains (<code class=\"text-blue-300\">TERMINAL_EMULATOR</code>)."
                    </p>
                </div>
            </Section>

            // ── Flutter SDK Settings ─────────────────────────────────
            <Section title="Flutter SDK Settings">
                <p class="text-slate-400">
                    "Override the Flutter SDK path. When not set, fdemon auto-detects via version managers and system PATH."
                </p>
                <CodeBlock language="toml" code="[flutter]
# Explicit Flutter SDK path override.
# Highest priority in the detection chain — bypasses all version manager detection.
# If not set, fdemon auto-detects via fvm, asdf, mise, puro, system PATH, etc.
sdk_path = \"/Users/me/flutter\"   # macOS / Linux example
# sdk_path = \"C:\\flutter\"       # Windows example" />
                <SettingsTable entries=vec![
                    ("sdk_path", "string", "(none)", "Explicit Flutter SDK path override. When set, this takes highest priority in the detection chain, bypassing fvm, asdf, mise, puro, and system PATH detection. Example: \"/Users/me/flutter\" or \"C:\\flutter\"."),
                ] />
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <p class="font-medium mb-1">"SDK Detection Order"</p>
                    <p class="mb-1">"When "<code class="text-blue-300">"sdk_path"</code>" is not set, fdemon tries these strategies in order:"</p>
                    <ol class="list-decimal list-inside space-y-1">
                        <li>"<code class=\"text-blue-300\">[flutter] sdk_path</code> in config.toml (this setting)"</li>
                        <li>"<code class=\"text-blue-300\">FLUTTER_ROOT</code> environment variable"</li>
                        <li>"FVM (<code class=\"text-blue-300\">.fvmrc</code> or <code class=\"text-blue-300\">.fvm/fvm_config.json</code>)"</li>
                        <li>"Puro (<code class=\"text-blue-300\">.puro.json</code>)"</li>
                        <li>"asdf / mise / proto (<code class=\"text-blue-300\">.tool-versions</code>, <code class=\"text-blue-300\">.mise.toml</code>, <code class=\"text-blue-300\">.prototools</code>)"</li>
                        <li>"System PATH (<code class=\"text-blue-300\">which flutter</code>)"</li>
                        <li>"Binary-only shim fallback"</li>
                    </ol>
                </div>
            </Section>

            // ── Complete Example ──────────────────────────────────────
            <Section title="Complete Example">
                <h3 class="text-lg font-bold text-white">"config.toml"</h3>
                <CodeBlock language="toml" code="[behavior]\nconfirm_quit = true\nauto_launch = false   # set true to auto-launch on cached last_device\n\n[watcher]\npaths = [\"lib\", \"packages/core/lib\"]\ndebounce_ms = 500\nauto_reload = true\nextensions = [\"dart\"]\n\n[ui]\nlog_buffer_size = 15000\nshow_timestamps = true\ncompact_logs = false\nstack_trace_collapsed = true\nstack_trace_max_frames = 3\n\n[devtools]\nauto_open = false\n\n[editor]\ncommand = \"\"  # Auto-detect" />

                <h3 class="text-lg font-bold text-white mt-6">"launch.toml"</h3>
                <CodeBlock language="toml" code="[[configurations]]\nname = \"Dev (iOS)\"\ndevice = \"iphone\"\nmode = \"debug\"\nflavor = \"development\"\nentry_point = \"lib/main_dev.dart\"\nauto_start = true\n\n[configurations.dart_defines]\nAPI_URL = \"https://dev.api.example.com\"\nDEBUG_MODE = \"true\"\n\n[[configurations]]\nname = \"Production\"\ndevice = \"auto\"\nmode = \"release\"\nflavor = \"production\"\nentry_point = \"lib/main_prod.dart\"\nextra_args = [\"--obfuscate\", \"--split-debug-info=build/symbols\"]\n\n[configurations.dart_defines]\nAPI_URL = \"https://api.example.com\"" />
            </Section>

            // ── Best Practices ───────────────────────────────────────
            <Section title="Best Practices">
                <div class="space-y-4">
                    <Tip title="Use launch configs for environments" text="Create separate configurations for dev/staging/prod instead of manually passing arguments." />
                    <Tip title="Keep secrets out of config files" text="Use extra_args = [\"--dart-define-from-file=secrets.json\"] for sensitive values. Don't commit API keys." />
                    <Tip title="Tune debounce for your project" text="Fast iterations: 300ms. Large projects: 1000ms to avoid reload spam during batch file changes." />
                    <Tip title="Set auto_start for your main config" text="Mark your primary development configuration with auto_start = true for instant startup." />
                    <Tip title="Opt into cache-based auto-launch deliberately" text="Set [behavior] auto_launch = true only if you want fdemon to silently relaunch your last-used device on every run. The default (false) keeps the cache as a dialog memory aid \u{2014} no surprises." />
                    <Tip title="Keep .vscode/launch.json for team compat" text="If your team uses VSCode, maintain launch.json alongside launch.toml. Flutter Demon imports both." />
                </div>
            </Section>
        </div>
    }
}

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
fn SettingsTable(
    entries: Vec<(&'static str, &'static str, &'static str, &'static str)>,
) -> impl IntoView {
    view! {
        <div class="overflow-hidden rounded-lg border border-slate-800">
            <table class="w-full text-left text-sm">
                <thead class="bg-slate-900 text-slate-200">
                    <tr>
                        <th class="p-4 font-medium">"Property"</th>
                        <th class="p-4 font-medium">"Default"</th>
                        <th class="p-4 font-medium hidden md:table-cell">"Description"</th>
                    </tr>
                </thead>
                <tbody class="divide-y divide-slate-800 bg-slate-950">
                    {entries.into_iter().map(|(prop, _typ, default, desc)| {
                        view! {
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">{prop}</td>
                                <td class="p-4 font-mono text-slate-300 whitespace-nowrap">{default}</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">{desc}</td>
                            </tr>
                        }
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}

#[component]
fn EditorRow(editor: &'static str, command: &'static str, pattern: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50">
            <td class="p-4 text-white font-medium">{editor}</td>
            <td class="p-4 font-mono text-blue-400">{command}</td>
            <td class="p-4 font-mono text-slate-500 text-xs hidden md:table-cell">{pattern}</td>
        </tr>
    }
}

#[component]
fn PropRow(prop: &'static str, typ: &'static str, desc: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50">
            <td class="p-4 font-mono text-blue-400 whitespace-nowrap">{prop}</td>
            <td class="p-4 font-mono text-slate-300">{typ}</td>
            <td class="p-4 text-slate-500 hidden md:table-cell">{desc}</td>
        </tr>
    }
}

#[component]
fn Tip(title: &'static str, text: &'static str) -> impl IntoView {
    view! {
        <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
            <h4 class="font-bold text-white mb-1">{title}</h4>
            <p class="text-sm text-slate-400">{text}</p>
        </div>
    }
}
