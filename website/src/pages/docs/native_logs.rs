use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::code_block::CodeBlock;

#[component]
pub fn NativeLogs() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Native Platform Logs"</h1>
            <p class="text-lg text-slate-400">
                "Flutter Demon automatically captures native platform logs alongside Flutter\u{2019}s Dart output \
                 \u{2014} giving you Kotlin, Swift, and Objective-C logs without leaving the terminal."
            </p>

            // ── Overview ──────────────────────────────────────────────
            <Section title="Overview">
                <p class="text-slate-400">
                    "Every Flutter app runs on a native layer. Native plugins, platform channels, and SDK \
                     libraries write logs using platform-specific APIs. Flutter Demon captures these alongside \
                     your Dart logs and merges them into a single unified stream."
                </p>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Android"</h4>
                        <p class="text-sm text-slate-400">
                            "Via "<code class="text-green-400">"adb logcat"</code>". Captures Kotlin, Java, \
                             and Go plugin logs from the app process."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"iOS"</h4>
                        <p class="text-sm text-slate-400">
                            "Via "<code class="text-green-400">"idevicesyslog"</code>" (physical) or "
                            <code class="text-green-400">"xcrun simctl log stream"</code>" (simulator). \
                             Captures Swift and Objective-C plugin logs."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"macOS"</h4>
                        <p class="text-sm text-slate-400">
                            "Via "<code class="text-green-400">"log stream"</code>". Captures "
                            <code class="text-green-400">"NSLog"</code>" and "
                            <code class="text-green-400">"os_log"</code>" from native plugins."
                        </p>
                    </div>
                </div>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Note:"</strong>
                    " Linux, Windows, and Web targets are already fully covered by stdout/stderr pipes. \
                     No additional native log capture is needed for those platforms."
                </div>
            </Section>

            // ── How It Works ──────────────────────────────────────────
            <Section title="How It Works">
                <p class="text-slate-400">
                    "Native log capture is fully automatic. You do not need to configure anything to get started."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Step"</th>
                                <th class="p-4 font-medium">"What Happens"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"1. App starts"</td>
                                <td class="p-4 text-slate-300">
                                    "Flutter Demon receives the "<code class="text-blue-400">"AppStarted"</code>
                                    " event and detects the target platform."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"2. Capture starts"</td>
                                <td class="p-4 text-slate-300">
                                    "The appropriate platform tool is spawned ("
                                    <code class="text-blue-400">"adb logcat"</code>", "
                                    <code class="text-blue-400">"idevicesyslog"</code>", or "
                                    <code class="text-blue-400">"log stream"</code>
                                    "). If the tool is not installed, capture is silently skipped."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"3. Tag discovery"</td>
                                <td class="p-4 text-slate-300">
                                    "As native log lines arrive, their tags are automatically extracted and \
                                     added to the tag filter overlay."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"4. Filtering"</td>
                                <td class="p-4 text-slate-300">
                                    "Two-tier filtering: config-level (min level, excluded tags) runs first; \
                                     UI-level (tag filter overlay) applies per-session at runtime."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"5. App stops"</td>
                                <td class="p-4 text-slate-300">
                                    "The capture process is terminated when the Flutter session ends."
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Supported Platforms ───────────────────────────────────
            <Section title="Supported Platforms">
                <p class="text-slate-400">
                    "Flutter Demon detects the target platform automatically and spawns the appropriate \
                     capture tool. No manual configuration is required."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"Android"</h3>
                <p class="text-slate-400">
                    "Uses "<code class="text-blue-400">"adb logcat --pid \u{003c}pid\u{003e}"</code>" to filter \
                     log output to the app process. Captures all tags including Kotlin, Java, and native \
                     plugin logs. The "<code class="text-blue-400">"flutter"</code>" tag is excluded by default \
                     to avoid duplicating logs already captured by the Flutter daemon."
                </p>
                <p class="text-slate-400">
                    "Requires: "<code class="text-blue-400">"adb"</code>" on PATH (part of the Android SDK platform-tools)."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"iOS Physical Device"</h3>
                <p class="text-slate-400">
                    "Uses "<code class="text-blue-400">"idevicesyslog -u \u{003c}udid\u{003e} -p Runner"</code>
                    " to stream system logs from the device. Captures Swift and Objective-C plugin output."
                </p>
                <p class="text-slate-400">
                    "Requires: "<code class="text-blue-400">"libimobiledevice"</code>" ("
                    <code class="text-blue-400">"brew install libimobiledevice"</code>" on macOS)."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"iOS Simulator"</h3>
                <p class="text-slate-400">
                    "Uses "<code class="text-blue-400">"xcrun simctl spawn \u{003c}udid\u{003e} log stream"</code>
                    " which hooks into the macOS Unified Logging system for the selected simulator."
                </p>
                <p class="text-slate-400">
                    "Requires: Xcode command-line tools (already present on macOS development machines)."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"macOS Desktop"</h3>
                <p class="text-slate-400">
                    "Uses "<code class="text-blue-400">"log stream --predicate 'process == \"<app>\"'"</code>
                    " to capture "<code class="text-blue-400">"NSLog"</code>" and "
                    <code class="text-blue-400">"os_log"</code>" output from the running app process."
                </p>
                <p class="text-slate-400">
                    "Requires: macOS 10.12 or later (Unified Logging is built in)."
                </p>

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Automatic fallback:"</strong>
                    " If the capture tool is not available or not installed, Flutter Demon silently skips \
                     native log capture for that session. No error is shown and the rest of fdemon \
                     continues to work normally."
                </div>
            </Section>

            // ── Tag Filter UI ─────────────────────────────────────────
            <Section title="Tag Filter UI">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"T"</code>" from the log view to open the tag filter \
                     overlay. It shows every tag discovered so far, with a count of how many log entries \
                     carry that tag."
                </p>

                <div class="bg-slate-900 rounded-lg border border-slate-800 p-4 font-mono text-xs text-slate-400 overflow-x-auto mt-4">
                    <pre class="leading-relaxed">{"\
\u{250c}\u{2500} Native Log Tag Filter \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}
\u{2502} [a] Show all  [n] Hide all  Space: toggle  Esc: close  \u{2502}
\u{251c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2524}
\u{2502} \u{2714} FlutterPlugin                                    142  \u{2502}
\u{2502} \u{2714} AudioManager                                      38  \u{2502}
\u{2502}   CameraX                                            21  \u{2502}
\u{2502} \u{2714} NetworkSecurity                                    15  \u{2502}
\u{2502}   SensorManager                                       8  \u{2502}
\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}"}</pre>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Controls"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="T" action="Open / close the tag filter overlay" />
                            <KeyRow key="\u{2191} / k" action="Move selection up in the tag list" />
                            <KeyRow key="\u{2193} / j" action="Move selection down in the tag list" />
                            <KeyRow key="Space" action="Toggle the selected tag on / off" />
                            <KeyRow key="a" action="Show all tags (enable every tag)" />
                            <KeyRow key="n" action="Hide all tags (disable every tag)" />
                            <KeyRow key="Esc" action="Close the overlay and return to normal mode" />
                        </tbody>
                    </table>
                </div>

                <p class="text-slate-400 mt-4">
                    "Tags with a checkmark "<code class="text-blue-400">"\u{2714}"</code>" are visible in the \
                     log view. Tags without a checkmark are hidden. The overlay is per-session \u{2014} each \
                     session has its own independent tag visibility state."
                </p>

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <p class="font-medium mb-1">"Tag discovery is incremental"</p>
                    <p>
                        "Tags appear in the overlay as they are first seen. New tags are added to the \
                         list automatically throughout the session without requiring a restart."
                    </p>
                </div>
            </Section>

            // ── Configuration ─────────────────────────────────────────
            <Section title="Configuration">
                <p class="text-slate-400">
                    "Native log capture is controlled by the "<code class="text-blue-400">"[native_logs]"</code>
                    " section of "<code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>
                    ". All settings are optional."
                </p>
                <CodeBlock language="toml" code="[native_logs]
enabled = true            # Enable / disable native log capture
min_level = \"debug\"       # Minimum log level: debug, info, warn, error
buffer_size = 1000        # Ring buffer size per session
exclude_tags = [\"flutter\"] # Tags to always exclude" />
                <SettingsTable entries=vec![
                    ("enabled", "boolean", "true", "Enable or disable native log capture globally"),
                    ("min_level", "string", "\"debug\"", "Minimum log level to capture. Options: debug, info, warn, error"),
                    ("buffer_size", "integer", "1000", "Number of native log entries to retain per session ring buffer"),
                    ("exclude_tags", "array<string>", r#"["flutter"]"#, "Tags that are always excluded from capture regardless of UI filter state"),
                ] />

                <h3 class="text-lg font-bold text-white mt-6">"Per-Tag Level Overrides"</h3>
                <p class="text-slate-400 mb-2">
                    "You can set a different minimum log level for individual tags. This is useful when \
                     a specific tag is too noisy at the default level:"
                </p>
                <CodeBlock language="toml" code="[native_logs.tag_levels]
AudioManager = \"warn\"     # Only warn/error from AudioManager
CameraX = \"error\"         # Only errors from CameraX
MyPlugin = \"debug\"        # All levels from MyPlugin" />

                <h3 class="text-lg font-bold text-white mt-6">"Custom Log Sources"</h3>
                <p class="text-slate-400 mb-2">
                    "Define arbitrary commands whose output is parsed as native log entries. Custom \
                     sources are started alongside platform capture and their tags appear in the tag \
                     filter overlay:"
                </p>
                <CodeBlock language="toml" code="[[native_logs.sources]]
name = \"AppLogs\"
command = \"tail\"
args = [\"-f\", \"/var/log/myapp.log\"]
format = \"raw\"

[[native_logs.sources]]
name = \"JsonService\"
command = \"journalctl\"
args = [\"-f\", \"-o\", \"json\", \"-u\", \"myservice\"]
format = \"json\"" />
            </Section>

            // ── Custom Log Sources ────────────────────────────────────
            <Section title="Custom Log Sources">
                <p class="text-slate-400">
                    "Custom sources let you pipe any command\u{2019}s output into the native log stream. Each \
                     source specifies a command, its arguments, and the line format to use for parsing."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"Format Options"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Format"</th>
                                <th class="p-4 font-medium">"Description"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Example Source"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"raw"</td>
                                <td class="p-4 text-slate-300">"Each line is treated as a plain message with info level."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"tail -f /var/log/app.log"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"json"</td>
                                <td class="p-4 text-slate-300">"Each line is parsed as JSON with level, tag, and message fields."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"journalctl -f -o json"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"logcat-threadtime"</td>
                                <td class="p-4 text-slate-300">"Android logcat threadtime format (date, time, pid, tid, level, tag, message)."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"adb logcat -v threadtime"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"syslog"</td>
                                <td class="p-4 text-slate-300">"BSD syslog format (timestamp, host, process, message)."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"log stream (macOS)"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Examples"</h3>
                <p class="text-slate-400 mb-2">"Tail a plain log file:"</p>
                <CodeBlock language="toml" code="[[native_logs.sources]]
name = \"AppLogs\"
command = \"tail\"
args = [\"-f\", \"/var/log/myapp/app.log\"]
format = \"raw\"" />

                <p class="text-slate-400 mb-2">"Stream structured JSON logs:"</p>
                <CodeBlock language="toml" code="[[native_logs.sources]]
name = \"JsonService\"
command = \"./scripts/log-stream.sh\"
args = []
format = \"json\"" />

                <p class="text-slate-400 mb-2">"Filtered logcat for specific tags:"</p>
                <CodeBlock language="toml" code="[[native_logs.sources]]
name = \"AudioOnly\"
command = \"adb\"
args = [\"logcat\", \"-s\", \"AudioManager:V\", \"MediaPlayer:V\"]
format = \"logcat-threadtime\"" />

                <div class="p-4 bg-slate-900 rounded-lg border border-slate-800 mt-2">
                    <h4 class="font-bold text-white mb-1">"Custom source tags in the filter overlay"</h4>
                    <p class="text-sm text-slate-400">
                        "Tags from custom sources appear in the tag filter overlay alongside platform \
                         tags. You can toggle them independently just like any other tag."
                    </p>
                </div>
            </Section>

            // ── Troubleshooting ───────────────────────────────────────
            <Section title="Troubleshooting">
                <div class="space-y-4">
                    <Tip
                        title="Native logs not appearing?"
                        text="Check that the capture tool is installed (adb, idevicesyslog, or xcrun). \
                               Verify the platform is supported and that enabled = true in [native_logs]. \
                               Check that min_level is not set higher than the level of your log messages."
                    />
                    <Tip
                        title="Too many tags in the overlay?"
                        text="Add noisy tags to exclude_tags in [native_logs] to permanently hide them. \
                               Use per-tag level overrides in [native_logs.tag_levels] to raise the \
                               minimum level for chatty tags without hiding them entirely."
                    />
                    <Tip
                        title="Duplicate logs appearing?"
                        text="The flutter tag is excluded by default to prevent duplication with Flutter \
                               daemon output. If you see duplicates for another tag, add it to exclude_tags. \
                               If you removed flutter from exclude_tags intentionally, you may see duplicates."
                    />
                    <Tip
                        title="Physical iOS device not capturing?"
                        text="Install libimobiledevice: brew install libimobiledevice. Make sure the \
                               device is trusted (unlocked and trust dialog accepted) and that idevicesyslog \
                               can see the device by running it manually first."
                    />
                    <Tip
                        title="Custom source command exits immediately?"
                        text="Custom source commands must stream continuously (e.g. tail -f, journalctl -f). \
                               If the command exits, capture stops for that source. Check the command runs \
                               correctly in a terminal before adding it to config."
                    />
                </div>

                <p class="text-slate-400 mt-4 text-sm">
                    "For the complete keybinding reference including the "<code class="text-blue-400">"T"</code>
                    " key, see the "
                    <A href="/docs/keybindings" attr:class="text-blue-400 hover:underline">"Keybindings"</A>
                    " page. For general configuration options, see "
                    <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>
                    "."
                </p>
            </Section>
        </div>
    }
}

// ── Local helper components ───────────────────────────────────────────────────

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
fn KeyRow(key: &'static str, action: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50 transition-colors">
            <td class="p-4 font-mono text-blue-400 whitespace-nowrap">{key}</td>
            <td class="p-4 text-slate-300">{action}</td>
        </tr>
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
fn Tip(title: &'static str, text: &'static str) -> impl IntoView {
    view! {
        <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
            <h4 class="font-bold text-white mb-1">{title}</h4>
            <p class="text-sm text-slate-400">{text}</p>
        </div>
    }
}
