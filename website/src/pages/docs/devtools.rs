use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Devtools() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"DevTools Integration"</h1>
            <p class="text-lg text-slate-400">
                "Built-in Flutter DevTools \u{2014} inspect widgets, explore layouts, and monitor performance \
                 without leaving the terminal."
            </p>

            // ── Overview ─────────────────────────────────────────────
            <Section title="Overview">
                <p class="text-slate-400">
                    "Flutter Demon integrates directly with the Flutter VM Service to bring essential DevTools \
                     panels into your terminal. While the full browser-based DevTools suite remains available \
                     for advanced workflows, the TUI panels cover the most common inspection tasks:"
                </p>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Widget Inspector"</h4>
                        <p class="text-sm text-slate-400">"Browse the live widget tree with expandable nodes, detailed properties, and flex layout data."</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Performance Monitor"</h4>
                        <p class="text-sm text-slate-400">"Real-time FPS sparkline, memory gauge, jank percentage, and GC history."</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Network Monitor"</h4>
                        <p class="text-sm text-slate-400">"Capture and inspect HTTP requests made by the app, with full headers, bodies, and timing breakdowns."</p>
                    </div>
                </div>
                <p class="text-slate-400 mt-4">
                    "DevTools integration works by connecting to the Flutter app's VM Service via WebSocket. \
                     The VM Service URI is printed by Flutter during startup and captured automatically. \
                     DevTools mode is only available when a session has an active debug-mode app running."
                </p>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Requirement:"</strong>
                    " Your Flutter app must be running in "<strong>"debug mode"</strong>
                    " for DevTools to connect. Profile and release builds do not expose the VM Service."
                </div>
            </Section>

            // ── Entering & Exiting ────────────────────────────────────
            <Section title="Entering and Exiting DevTools">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"d"</code>" from Normal mode to enter DevTools mode. \
                     The log view is replaced by the DevTools panel area. The app header and session tabs remain \
                     visible above the panels so you can switch sessions without leaving DevTools mode."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="d" action="Enter DevTools mode (from Normal mode)" />
                            <KeyRow key="Esc" action="Return to log view" />
                            <KeyRow key="i" action="Switch to Widget Inspector panel" />
                            <KeyRow key="p" action="Switch to Performance Monitor panel" />
                            <KeyRow key="n" action="Switch to Network Monitor panel" />
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 mt-4">
                    "If the active session does not have a VM Service connection yet (e.g. the app is still \
                     starting up), DevTools mode displays a connection status message. It transitions to the \
                     active panel automatically once the connection is established."
                </p>
            </Section>

            // ── Widget Inspector ──────────────────────────────────────
            <Section title="Widget Inspector (i)">
                <p class="text-slate-400">
                    "The Widget Inspector fetches the live widget tree from the running app and displays it \
                     as a navigable tree. Press "<code class="text-blue-400">"i"</code>" while in DevTools mode \
                     to open this panel."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"Layout"</h3>
                <p class="text-slate-400">
                    "The inspector is split into two areas: the widget tree on the left (approximately 60% of the \
                     width) and a details panel on the right (40%). In narrow terminals the two areas stack \
                     vertically with the tree above the details."
                </p>
                <div class="bg-slate-900 rounded-lg border border-slate-800 p-4 font-mono text-xs text-slate-400 overflow-x-auto mt-2">
                    <pre class="leading-relaxed">{"\
\u{250c}\u{2500} Widget Inspector \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}
\u{2502} MaterialApp                     \u{2502} Type: Scaffold               \u{2502}
\u{2502} \u{2514}\u{2500} Scaffold                     \u{2502} Creator: main.dart:42       \u{2502}
\u{2502}   \u{251c}\u{2500} AppBar                      \u{2502}                             \u{2502}
\u{2502}   \u{2514}\u{2500}\u{25b6} Column (selected)           \u{2502} Constraints:                \u{2502}
\u{2502}       \u{251c}\u{2500} Text                        \u{2502}   w: 0..390, h: 0..844     \u{2502}
\u{2502}       \u{2514}\u{2500} ElevatedButton              \u{2502} Size: 390.0 \u{00d7} 800.0         \u{2502}
\u{2502}                               \u{2502}                             \u{2502}
\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}"}</pre>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Navigation"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="\u{2191} / k" action="Move selection up" />
                            <KeyRow key="\u{2193} / j" action="Move selection down" />
                            <KeyRow key="\u{2192} / Enter" action="Expand selected node" />
                            <KeyRow key="\u{2190} / h" action="Collapse selected node" />
                            <KeyRow key="r" action="Re-fetch the widget tree from the VM" />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Details Panel"</h3>
                <p class="text-slate-400">
                    "When a widget is selected the details panel shows:"
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mt-2">
                    <li>"Widget type (e.g. "<code class="text-blue-400">"Scaffold"</code>")"</li>
                    <li>"A short description from the widget itself"</li>
                    <li>"Creation location: file path and line number"</li>
                    <li>"Render object constraints and actual size"</li>
                </ul>
                <p class="text-slate-400 mt-3">
                    "Widgets from your own code are highlighted differently from framework widgets, making \
                     it easy to identify the boundaries between your code and the Flutter SDK."
                </p>
            </Section>

            // ── Layout Explorer ───────────────────────────────────────
            <Section title="Layout Explorer (l)">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"l"</code>" to open the Layout Explorer. It displays \
                     flex layout data for the widget currently selected in the Inspector. If no widget is \
                     selected, it prompts you to select one first."
                </p>
                <p class="text-slate-400 mt-3">
                    "The Layout Explorer auto-fetches layout data whenever you switch to it (provided a widget \
                     is already selected), so you always see up-to-date information."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"What Is Shown"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Section"</th>
                                <th class="p-4 font-medium">"Details"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Constraints"</td>
                                <td class="p-4 text-slate-300">"Min/max width and height. Tight constraints are highlighted."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Size"</td>
                                <td class="p-4 text-slate-300">"Actual rendered width and height, proportionally visualized."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Flex Properties"</td>
                                <td class="p-4 text-slate-300">"mainAxisAlignment, crossAxisAlignment, flex factor, and FlexFit."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Children"</td>
                                <td class="p-4 text-slate-300">"Individual child sizes and flex allocations within the parent."</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 mt-3">
                    "This panel is most useful for debugging "<strong class="text-white">"overflow errors"</strong>
                    " and understanding why a widget renders at a particular size."
                </p>
            </Section>

            // ── Performance Monitor ───────────────────────────────────
            <Section title="Performance Monitor (p)">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"p"</code>" to open the Performance Monitor. Unlike the \
                     Inspector and Layout Explorer, performance data streams continuously in real time \
                     \u{2014} no manual refresh is required."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"FPS Sparkline"</h3>
                <p class="text-slate-400">
                    "A rolling 300-frame sparkline shows the recent frame rate history. Bars are color-coded:"
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mt-2">
                    <li><span class="text-green-400">"Green"</span>" \u{2014} 55+ FPS (smooth)"</li>
                    <li><span class="text-yellow-400">"Yellow"</span>" \u{2014} 30\u{2013}55 FPS (acceptable)"</li>
                    <li><span class="text-red-400">"Red"</span>" \u{2014} below 30 FPS (janky)"</li>
                </ul>

                <h3 class="text-lg font-bold text-white mt-6">"Memory Gauge"</h3>
                <p class="text-slate-400">
                    "The memory gauge shows current heap usage against the heap capacity. Three values are \
                     displayed: used heap, capacity, and external memory (native allocations outside the \
                     Dart heap)."
                </p>

                <h3 class="text-lg font-bold text-white mt-6">"Stats Panel"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Metric"</th>
                                <th class="p-4 font-medium">"Description"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Frame Count"</td>
                                <td class="p-4 text-slate-300">"Total frames rendered since the monitor started."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Jank %"</td>
                                <td class="p-4 text-slate-300">"Percentage of frames that exceeded the 16.6 ms budget."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"P95 Frame Time"</td>
                                <td class="p-4 text-slate-300">"95th-percentile frame duration in milliseconds."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Avg Frame Time"</td>
                                <td class="p-4 text-slate-300">"Mean frame duration across the rolling window."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"GC Events"</td>
                                <td class="p-4 text-slate-300">"Recent garbage collection events with type and reclaimed bytes."</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 mt-3">
                    "The polling interval for memory data is controlled by "<code class="text-blue-400">"performance_refresh_ms"</code>
                    " in the "<code class="text-blue-400">"[devtools]"</code>" config section. Frame timing and GC events \
                     are always streamed in real time regardless of this setting."
                </p>
            </Section>

            // ── Network Monitor ───────────────────────────────────────
            <Section title="Network Monitor (n)">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"n"</code>" while in DevTools mode to open the Network \
                     Monitor. It captures HTTP requests made by the Flutter app via the VM Service \
                     "<code class="text-blue-400">"ext.dart.io.getHttpProfile"</code>" extension. Recording \
                     must be enabled to capture new requests."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"Request Table"</h3>
                <p class="text-slate-400">
                    "The main view shows a table of captured HTTP requests with the following columns: \
                     Method, URL (truncated to fit), Status Code, Duration, and Size. Navigate rows with \
                     "<code class="text-blue-400">"j/k"</code>" or the arrow keys (single row), or "
                    <code class="text-blue-400">"PgUp/PgDn"</code>" to jump a page of 10 at a time. \
                     Status codes are color-coded: "
                    <span class="text-green-400">"green"</span>" for 2xx, "
                    <span class="text-yellow-400">"yellow"</span>" for 3xx, "
                    <span class="text-red-400">"red"</span>" for 4xx and 5xx."
                </p>
                <div class="bg-slate-900 rounded-lg border border-slate-800 p-4 font-mono text-xs text-slate-400 overflow-x-auto mt-2">
                    <pre class="leading-relaxed">{"\
\u{250c}\u{2500} Network Monitor \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}
\u{2502} \u{25cf} Recording   12 requests   Filter: none                          \u{2502}
\u{2502}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2502}
\u{2502} GET  /api/users           200  45ms   1.2 KB                      \u{2502}
\u{2502} POST /api/login           200  120ms  0.3 KB                      \u{2502}
\u{2502} GET  /api/posts?page=1    200  89ms   4.5 KB                      \u{2502}
\u{2502}\u{25b6}GET  /api/posts/42        404  23ms   0.1 KB                      \u{2502}
\u{2502} GET  /api/config          200  12ms   0.8 KB                      \u{2502}
\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}"}</pre>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Request Detail View"</h3>
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"Enter"</code>" on any request to open the detail \
                     view. Five sub-tabs are accessible via single-key shortcuts:"
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-2">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Tab"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="g" action="General \u{2014} method, URL, status, start time, duration, content type" />
                            <KeyRow key="h" action="Headers \u{2014} request and response headers in key-value format" />
                            <KeyRow key="q" action="Request Body \u{2014} payload sent with the request (if any)" />
                            <KeyRow key="s" action="Response Body \u{2014} response content" />
                            <KeyRow key="t" action="Timing \u{2014} connection timing breakdown (DNS, connect, TLS, first byte, transfer)" />
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 mt-3">
                    "Press "<code class="text-blue-400">"Esc"</code>" to deselect the request and return \
                     to the list view. Note that in the Network panel, "<code class="text-blue-400">"q"</code>
                    " switches to the Request Body tab rather than quitting \u{2014} use "
                    <code class="text-blue-400">"Esc"</code>" then "<code class="text-blue-400">"q"</code>
                    " (or "<code class="text-blue-400">"Esc"</code>" twice) to exit DevTools mode."
                </p>

                <h3 class="text-lg font-bold text-white mt-6">"Recording and Controls"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="Space" action="Toggle recording on/off. When recording is off, no new requests are captured." />
                            <KeyRow key="Ctrl+X" action="Clear all recorded requests from history." />
                            <KeyRow key="/" action="Enter filter mode to filter requests by URL substring." />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Filter Mode"</h3>
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"/"</code>" to enter a text input mode at the top \
                     of the panel. Type to filter requests \u{2014} only URLs that contain the typed substring \
                     are shown in the table. Press "<code class="text-blue-400">"Enter"</code>" to apply the \
                     filter or "<code class="text-blue-400">"Esc"</code>" to cancel. The filter persists \
                     until cleared."
                </p>

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Requirement:"</strong>
                    " Network profiling requires a "<strong>"debug-mode"</strong>" app with an active VM Service \
                     connection. The HTTP profile extension is not available in profile or release builds."
                </div>
            </Section>

            // ── Debug Overlays ────────────────────────────────────────
            <Section title="Debug Overlays">
                <p class="text-slate-400">
                    "Debug overlays are rendered on the device/emulator screen itself, not in the terminal. \
                     Flutter Demon sends toggle commands to the running app via the VM Service. Active overlays \
                     are shown as indicators in the DevTools tab bar."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Shortcut"</th>
                                <th class="p-4 font-medium">"Overlay"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"What It Shows"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Ctrl+r"</td>
                                <td class="p-4 text-white font-medium">"Repaint Rainbow"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Highlights widgets that are repainting with rotating colors. Identifies unnecessary rebuilds."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Ctrl+p"</td>
                                <td class="p-4 text-white font-medium">"Performance Overlay"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Shows GPU and UI thread timing bars at the top of the screen."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"Ctrl+d"</td>
                                <td class="p-4 text-white font-medium">"Debug Paint"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Draws colored borders around widget boundaries and highlights padding, margins, and baselines."</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <p class="font-medium mb-1">"Tip: Auto-enable on connect"</p>
                    <p>
                        "Set "<code class="text-blue-400">"auto_repaint_rainbow = true"</code>" or "
                        <code class="text-blue-400">"auto_performance_overlay = true"</code>
                        " in your "<code class="text-blue-400">"[devtools]"</code>" config to activate these \
                         overlays automatically every time the VM Service connects."
                    </p>
                </div>
            </Section>

            // ── Browser DevTools ──────────────────────────────────────
            <Section title="Browser DevTools (b)">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"b"</code>" while in DevTools mode to open Flutter \
                     DevTools in your browser. Flutter Demon uses the VM Service URI to construct the correct \
                     local DDS (Dart Development Service) URL and passes it to your browser."
                </p>
                <p class="text-slate-400 mt-3">
                    "The browser-based DevTools suite offers additional tools not available in the TUI, including:"
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mt-2">
                    <li>"Timeline / frame rendering detail"</li>
                    <li>"Network inspector"</li>
                    <li>"Memory allocation profiler"</li>
                    <li>"CPU profiler"</li>
                    <li>"Logging tab with structured filtering"</li>
                </ul>

                <h3 class="text-lg font-bold text-white mt-6">"Configuring a Browser"</h3>
                <p class="text-slate-400 mb-2">
                    "By default Flutter Demon uses the system default browser. To specify a browser explicitly, \
                     set the "<code class="text-blue-400">"browser"</code>" option in "
                    <code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>":"
                </p>
                <CodeBlock language="toml" code="[devtools]\nbrowser = \"chrome\"         # or \"firefox\", \"safari\", \"open\", full path, etc.\nbrowser = \"\"              # empty = system default" />
            </Section>

            // ── Connection States ─────────────────────────────────────
            <Section title="Connection States">
                <p class="text-slate-400">
                    "The VM Service connection passes through several states during a session lifetime. \
                     DevTools panels reflect the current state with informative messages."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"State"</th>
                                <th class="p-4 font-medium">"Description"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Panel Behavior"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-green-400 whitespace-nowrap">"Connected"</td>
                                <td class="p-4 text-white">"VM Service WebSocket is open and responding."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Normal operation. All panels fetch and stream data."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-yellow-400 whitespace-nowrap">"Connecting"</td>
                                <td class="p-4 text-white">"Initial connection attempt in progress."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Panels show a loading indicator. Transitions automatically when ready."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-yellow-400 whitespace-nowrap">"Reconnecting"</td>
                                <td class="p-4 text-white">"Connection was lost; auto-reconnect with exponential backoff is in progress."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Panels show a reconnecting indicator with the attempt count."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-red-400 whitespace-nowrap">"Disconnected"</td>
                                <td class="p-4 text-white">"All reconnect attempts have failed or the app exited."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Panels show a disconnected message with instructions to restart the session."</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-orange-400 whitespace-nowrap">"Timeout"</td>
                                <td class="p-4 text-white">"A request to the VM Service did not complete in time."</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"The request is retried. The panel shows a timeout warning without crashing."</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <p class="font-medium mb-1">"Reconnection Strategy"</p>
                    <p>
                        "When the VM Service WebSocket drops (e.g. during a hot restart), Flutter Demon \
                         automatically reconnects using exponential backoff starting at 500 ms. The connection \
                         is re-established transparently \u{2014} you do not need to exit and re-enter DevTools mode."
                    </p>
                </div>
            </Section>

            // ── Configuration ─────────────────────────────────────────
            <Section title="Configuration">
                <p class="text-slate-400">
                    "DevTools behavior is controlled by the "<code class="text-blue-400">"[devtools]"</code>
                    " section of "<code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>
                    ". All settings are optional and have sensible defaults."
                </p>
                <CodeBlock language="toml" code="[devtools]
auto_open = false
browser = \"\"
default_panel = \"inspector\"
performance_refresh_ms = 2000
memory_history_size = 60
tree_max_depth = 0
auto_repaint_rainbow = false
auto_performance_overlay = false

[devtools.logging]
hybrid_enabled = true
prefer_vm_level = true
show_source_indicator = false
dedupe_threshold_ms = 100" />

                <h3 class="text-lg font-bold text-white mt-4">"All Settings"</h3>
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
                            <SettingsRow prop="auto_open" default="false" desc="Automatically open DevTools in a browser when the app starts." />
                            <SettingsRow prop="browser" default="\"\"" desc="Browser command (e.g. \"chrome\", \"firefox\"). Empty string uses the system default." />
                            <SettingsRow prop="default_panel" default="\"inspector\"" desc="Panel shown when entering DevTools mode. Options: \"inspector\", \"layout\", \"performance\"." />
                            <SettingsRow prop="performance_refresh_ms" default="2000" desc="Memory data polling interval in milliseconds. Frame timing and GC events are always real-time." />
                            <SettingsRow prop="memory_history_size" default="60" desc="Number of memory snapshots retained in the ring buffer for the memory graph." />
                            <SettingsRow prop="tree_max_depth" default="0" desc="Max depth when fetching the widget tree. 0 fetches the entire tree." />
                            <SettingsRow prop="auto_repaint_rainbow" default="false" desc="Automatically enable the repaint rainbow overlay when the VM Service connects." />
                            <SettingsRow prop="auto_performance_overlay" default="false" desc="Automatically enable the performance overlay when the VM Service connects." />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Logging Settings"</h3>
                <p class="text-slate-400 text-sm mb-3">
                    "The "<code class="text-blue-400">"[devtools.logging]"</code>" sub-section controls how logs \
                     from the VM Service are merged with Flutter daemon logs."
                </p>
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
                            <SettingsRow prop="hybrid_enabled" default="true" desc="Merge VM Service logs with daemon stdout logs for a single unified stream." />
                            <SettingsRow prop="prefer_vm_level" default="true" desc="Use the log level reported by the VM Service (accurate) instead of content-based detection." />
                            <SettingsRow prop="show_source_indicator" default="false" desc="Show [VM] or [daemon] tags next to each log entry to indicate its source." />
                            <SettingsRow prop="dedupe_threshold_ms" default="100" desc="Logs from both sources within this window (ms) with matching content are deduplicated." />
                        </tbody>
                    </table>
                </div>

                <p class="text-slate-400 mt-4 text-sm">
                    "Press "<code class="text-blue-400">"," </code>" (comma) from Normal mode to open the \
                     built-in settings panel for live editing without touching files. See the "
                    <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>
                    " page for full details on all config sections."
                </p>
            </Section>

            // ── Keybindings Quick Reference ───────────────────────────
            <Section title="Keybindings Quick Reference">
                <p class="text-slate-400">
                    "All DevTools keybindings are active while in DevTools mode (entered with "
                    <code class="text-blue-400">"d"</code>" from Normal mode)."
                </p>

                <h3 class="text-lg font-bold text-white mt-4">"Panel Navigation"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="d" action="Enter DevTools mode (from Normal mode)" />
                            <KeyRow key="Esc" action="Exit DevTools mode, return to log view" />
                            <KeyRow key="i" action="Open Widget Inspector panel" />
                            <KeyRow key="p" action="Open Performance Monitor panel" />
                            <KeyRow key="n" action="Open Network Monitor panel" />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Widget Inspector Navigation"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="\u{2191} / k" action="Move selection up in the widget tree" />
                            <KeyRow key="\u{2193} / j" action="Move selection down in the widget tree" />
                            <KeyRow key="\u{2192} / Enter" action="Expand selected node" />
                            <KeyRow key="\u{2190} / h" action="Collapse selected node" />
                            <KeyRow key="r" action="Refresh widget tree from VM" />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Performance Monitor"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="j / \u{2193}" action="Scroll frame list down" />
                            <KeyRow key="k / \u{2191}" action="Scroll frame list up" />
                            <KeyRow key="s" action="Sort frames by duration" />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Network Monitor"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="j / \u{2193}" action="Move selection down in request list" />
                            <KeyRow key="k / \u{2191}" action="Move selection up in request list" />
                            <KeyRow key="PgDn" action="Page down (10 rows)" />
                            <KeyRow key="PgUp" action="Page up (10 rows)" />
                            <KeyRow key="Enter" action="Open request detail view" />
                            <KeyRow key="Esc" action="Close request detail, return to list" />
                            <KeyRow key="g" action="Detail: General tab" />
                            <KeyRow key="h" action="Detail: Headers tab" />
                            <KeyRow key="q" action="Detail: Request Body tab" />
                            <KeyRow key="s" action="Detail: Response Body tab" />
                            <KeyRow key="t" action="Detail: Timing tab" />
                            <KeyRow key="Space" action="Toggle recording on/off" />
                            <KeyRow key="Ctrl+X" action="Clear all request history" />
                            <KeyRow key="/" action="Enter URL filter mode" />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Debug Overlays"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="Ctrl+r" action="Toggle repaint rainbow on device/emulator" />
                            <KeyRow key="Ctrl+p" action="Toggle performance overlay on device/emulator" />
                            <KeyRow key="Ctrl+d" action="Toggle debug paint on device/emulator" />
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"Browser"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="b" action="Open Flutter DevTools in browser" />
                        </tbody>
                    </table>
                </div>

                <p class="text-slate-400 text-sm mt-4">
                    "For the complete keybinding reference across all modes, see the "
                    <A href="/docs/keybindings" attr:class="text-blue-400 hover:underline">"Keybindings"</A>
                    " page."
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
fn SettingsRow(
    prop: &'static str,
    default: &'static str,
    desc: &'static str,
) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50">
            <td class="p-4 font-mono text-blue-400 whitespace-nowrap">{prop}</td>
            <td class="p-4 font-mono text-slate-300 whitespace-nowrap">{default}</td>
            <td class="p-4 text-slate-500 hidden md:table-cell">{desc}</td>
        </tr>
    }
}
