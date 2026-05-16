use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Mouse() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Mouse Interactions"</h1>
            <p class="text-slate-400">
                "Flutter Demon supports mouse interaction in the terminal when "
                <code class="text-blue-400 bg-slate-900 px-1 rounded">"[ui] enable_mouse = true"</code>
                " (the default). This page describes how to select and copy log text, how the wheel "
                "scrolls each UI mode, the modifier keys that change scroll behavior, the runtime toggle, "
                "and the platform caveats. For the on/off setting, see "
                <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>"."
            </p>

            // ── Selecting and Copying Log Text ───────────────────────
            <Section title="Selecting and Copying Log Text">
                <p class="text-slate-400 mb-4">
                    "Three affordances are available for getting log text onto your clipboard:"
                </p>

                <h3 class="text-base font-semibold text-white mb-2">
                    "Shift+drag \u{2014} arbitrary substring selection"
                </h3>
                <p class="text-slate-400 mb-4">
                    "Hold "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift"</kbd>
                    " and drag the mouse to select any run of characters in the log view. The terminal's "
                    "native selection engine handles the highlight and the copy; "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Cmd+C"</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Shift+C"</kbd>
                    " (or your terminal's copy shortcut) copies the selection."
                </p>
                <p class="text-slate-400 mb-4">
                    "This works because fdemon no longer requests the "
                    <code class="text-blue-400">"?1003"</code>
                    " (any-motion) mouse-tracking mode. With only "
                    <code class="text-blue-400">"?1000"</code>
                    "/"
                    <code class="text-blue-400">"?1002"</code>
                    " enabled, modern terminals pass "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+drag"</kbd>
                    " through to their native selection handler. See "
                    <a href="#platform-caveats" class="text-blue-400 hover:underline">"Platform Caveats"</a>
                    " if Shift+drag still misbehaves in your terminal."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">
                    "Right-click \u{2014} full-line copy with toast confirmation"
                </h3>
                <p class="text-slate-400 mb-4">
                    "Right-click on any log row to copy that entry's complete text to the system clipboard. "
                    "A one-second status-bar toast confirms the copy: "
                    <code class="text-blue-400">"Copied: \u{003c}60-char preview\u{2026}\u{003e}"</code>
                    "."
                </p>
                <p class="text-slate-400 mb-4">
                    "Right-clicking outside a log row (e.g., on the header or a DevTools panel) shows a brief "
                    "informational toast and takes no action."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">
                    <code class="text-blue-400">"Alt+m"</code>
                    " runtime toggle \u{2014} fully suspend mouse capture"
                </h3>
                <p class="text-slate-400 mb-2">
                    "If Shift+drag still does not select text in your terminal, press "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    " to suspend mouse capture entirely. While capture is off:"
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mb-4">
                    <li>
                        "All mouse events go directly to the terminal \u{2014} native text selection and scrollback "
                        "work as if fdemon were a non-mouse-aware program."
                    </li>
                    <li>
                        "The status bar shows "
                        <code class="text-blue-400">"[mouse-off]"</code>
                        " (in warning color) so you know capture is paused."
                    </li>
                </ul>
                <p class="text-slate-400">
                    "Press "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    " again to restore fdemon's mouse features (scroll wheel, clickable header, "
                    "session tabs, DevTools panels, etc.). The toggle is in-process only; on restart, capture "
                    "returns to the state set by "
                    <code class="text-blue-400">"[ui] enable_mouse"</code>
                    " in your config file."
                </p>
            </Section>

            // ── Scroll Behavior ──────────────────────────────────────
            <Section title="Scroll Behavior by UI Mode">
                <p class="text-slate-400">
                    "Wheel events route to the focused surface based on the current UI mode. There is "
                    <strong class="text-white">"no coordinate-based routing"</strong>
                    " — scrolling anywhere in the terminal scrolls the focused surface (the log view, "
                    "the settings list, the active DevTools panel, etc.)."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Mode"</th>
                                <th class="p-4 font-medium">"Plain Wheel"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Shift+Wheel"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <ScrollRow mode="Normal (logs)" plain="Scroll log line up/down" shift="Page log up/down" />
                            <ScrollRow mode="Normal (tag-filter open)" plain="Move tag-filter selection" shift="Move tag-filter selection" />
                            <ScrollRow mode="LinkHighlight" plain="Scroll log line up/down" shift="Page log up/down" />
                            <ScrollRow mode="DevTools — Inspector" plain="Tree row up/down" shift="Tree row up/down (single-step; no page analogue)" />
                            <ScrollRow mode="DevTools — Performance" plain="(no-op — use ← / → for frame navigation)" shift="(no-op)" />
                            <ScrollRow mode="DevTools — Network" plain="Request list up/down" shift="Page request list up/down" />
                            <ScrollRow mode="DevTools — Network (filter input active)" plain="(no-op — text input mode)" shift="(no-op)" />
                            <ScrollRow mode="Settings (main list)" plain="Item selection up/down" shift="Item selection up/down" />
                            <ScrollRow mode="Settings (inline editing)" plain="(no-op — text input mode)" shift="(no-op)" />
                            <ScrollRow mode="Settings (dart-defines modal — list pane)" plain="Dart-define selection up/down" shift="Dart-define selection up/down" />
                            <ScrollRow mode="Settings (dart-defines modal — edit pane)" plain="(no-op — text input mode)" shift="(no-op)" />
                            <ScrollRow mode="Settings (extra-args modal)" plain="Extra-args selection up/down" shift="Extra-args selection up/down" />
                            <ScrollRow mode="NewSessionDialog (target-selector pane)" plain="Device selection up/down" shift="Device selection up/down" />
                            <ScrollRow mode="NewSessionDialog (launch-context pane)" plain="Field focus prev/next" shift="Field focus prev/next" />
                            <ScrollRow mode="NewSessionDialog (fuzzy modal open)" plain="Fuzzy selection up/down" shift="Fuzzy selection up/down" />
                            <ScrollRow mode="NewSessionDialog (dart-defines modal open)" plain="Dart-define selection up/down" shift="Dart-define selection up/down" />
                            <ScrollRow mode="FlutterVersion" plain="Version selection up/down" shift="Version selection up/down" />
                            <ScrollRow mode="SearchInput, Confirm, Loading, EmulatorSelector" plain="(no-op — text input or system modal)" shift="(no-op)" />
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-500 text-sm mt-2">
                    <strong class="text-slate-400">"Practical implication:"</strong>
                    " If you hover over a session tab and scroll, the log view scrolls — the tab strip does not change. "
                    "Use the keyboard ("
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"1"</kbd>
                    "\u{2013}"
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"9"</kbd>
                    " to jump to a session, "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"["</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"]"</kbd>
                    " to cycle) to switch sessions, or left-click a tab."
                </p>
            </Section>

            // ── Modifier Key Rules ───────────────────────────────────
            <Section title="Modifier Key Rules">
                <p class="text-slate-400 mb-4">
                    "The exact behavior of "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl"</kbd>
                    ", "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt"</kbd>
                    ", and "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift"</kbd>
                    " modifiers depends on the mode:"
                </p>

                <h3 class="text-base font-semibold text-white mb-2">
                    "Modes that honor Shift+Wheel for page-step scrolling"
                    <span class="text-slate-500 font-normal text-sm ml-2">"(Normal, LinkHighlight, DevTools/Network)"</span>
                </h3>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mb-4">
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+Wheel"</kbd>
                        " \u{2192} page up / page down"
                    </li>
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Shift+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Shift+Wheel"</kbd>
                        " \u{2192} "
                        <strong class="text-white">"no-op"</strong>
                        " (avoids conflict with terminal-level Ctrl+Wheel font-zoom bindings)"
                    </li>
                </ul>

                <h3 class="text-base font-semibold text-white mb-2">
                    "Modes that ignore modifiers"
                    <span class="text-slate-500 font-normal text-sm ml-2">"(Settings, NewSessionDialog, FlutterVersion)"</span>
                </h3>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mb-4">
                    <li>"All modifier combinations produce the same single-step navigation as plain wheel."</li>
                    <li>"There is no page-step analogue in these modes."</li>
                </ul>

                <h3 class="text-base font-semibold text-white mb-2">
                    "DevTools \u{2014} Inspector"
                    <span class="text-slate-500 font-normal text-sm ml-2">"(special case)"</span>
                </h3>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mb-4">
                    <li>
                        "Plain wheel, "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+Wheel"</kbd>
                        " \u{2192} single-step tree row navigation (no page-step analogue)"
                    </li>
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Wheel"</kbd>
                        " alone, "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Wheel"</kbd>
                        " alone \u{2192} no-op"
                    </li>
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Shift+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Shift+Wheel"</kbd>
                        " \u{2192} single-step navigation (Shift is prioritized over the no-op Ctrl/Alt rule)"
                    </li>
                </ul>
                <div class="bg-slate-900/50 border border-slate-700 rounded-lg p-4 mb-4">
                    <p class="text-slate-400 text-sm">
                        <strong class="text-white">"Note on Inspector modifier behavior:"</strong>
                        " The Inspector's handling of "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+Ctrl+Wheel"</kbd>
                        " (single-step rather than no-op) is a deliberate exception to the no-op rule used in "
                        "Normal/Network modes. Because the Inspector has no page-step navigation, Shift-held scrolls "
                        "fall through to single-step rather than becoming dead input."
                    </p>
                </div>

                <h3 class="text-base font-semibold text-white mb-2">"Horizontal Scroll"</h3>
                <p class="text-slate-400">
                    <code class="text-blue-400">"ScrollDir::Left"</code>
                    " and "
                    <code class="text-blue-400">"ScrollDir::Right"</code>
                    " (touchpad horizontal scroll) are "
                    <strong class="text-white">"no-ops in all modes"</strong>
                    ". Future phases may map horizontal scroll to log timeline panning or a DevTools secondary-axis navigation."
                </p>
            </Section>

            // ── Coordinate-Free Routing ──────────────────────────────
            <Section title="Coordinate-Free Routing">
                <p class="text-slate-400 mb-4">
                    "Wheel events are routed by "
                    <code class="text-blue-400">"UiMode"</code>
                    " only \u{2014} the cursor position "
                    <code class="text-blue-400">"(x, y)"</code>
                    " does not affect which surface receives the scroll. This means scrolling while hovering over the header, "
                    "status bar, or session tabs still scrolls the focused surface (e.g., the log view in "
                    <code class="text-blue-400">"Normal"</code>
                    " mode)."
                </p>
                <p class="text-slate-400 mb-4">
                    "This is a deliberate simplification. Scroll routing is coordinate-free; only "
                    <strong class="text-white">"click"</strong>
                    " events use the per-frame region registry for coordinate-based hit-testing (see Phases 3 and 4 below)."
                </p>
                <p class="text-slate-400">
                    <strong class="text-white">"Practical implication:"</strong>
                    " If you hover over a session tab and scroll, the log view scrolls \u{2014} the tab strip does not change. "
                    "Use the keyboard ("
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"1"</kbd>
                    "\u{2013}"
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"9"</kbd>
                    " to jump to a session, "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"["</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"]"</kbd>
                    " to cycle) to switch sessions, or left-click a tab to select it."
                </p>
            </Section>

            // ── Phase 3: Header and Session Tabs ─────────────────────
            <Section title="Phase 3: Header and Session Tab Clicks">
                <h3 class="text-base font-semibold text-white mb-2">"Header Shortcuts"</h3>
                <p class="text-slate-400 mb-4">
                    "Bracketed shortcuts in the title bar are clickable. Clicking "
                    <code class="text-blue-400">"[r]"</code>
                    ", "
                    <code class="text-blue-400">"[R]"</code>
                    ", "
                    <code class="text-blue-400">"[x]"</code>
                    ", "
                    <code class="text-blue-400">"[d]"</code>
                    ", "
                    <code class="text-blue-400">"[D]"</code>
                    ", or "
                    <code class="text-blue-400">"[q]"</code>
                    " fires the same action as the corresponding key, subject to the same busy-gate "
                    "(e.g. "
                    <code class="text-blue-400">"[r]"</code>
                    " is a no-op during a hot-reload in progress)."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"Session Tabs"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Click"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Left-click a tab"</td>
                                <td class="p-4 text-slate-400">"Switch to that session"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Middle-click a tab"</td>
                                <td class="p-4 text-slate-400">"Close that session"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Click the device pill (compact header)"</td>
                                <td class="p-4 text-slate-400">"Open the New Session dialog to add or switch devices"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Phase 4: Log View and DevTools ───────────────────────
            <Section title="Phase 4: Log View and DevTools Clicks">
                <h3 class="text-base font-semibold text-white mb-2">"Log View"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800 mb-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Interaction"</th>
                                <th class="p-4 font-medium">"Result"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Single click on a log row"</td>
                                <td class="p-4 text-slate-400">"No visible action; row is registered for double-click detection"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Double click on the same row within 400 ms"</td>
                                <td class="p-4 text-slate-400">"Toggles the entry's stack trace expansion (if it has a stack trace)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Double click on a different row within 400 ms"</td>
                                <td class="p-4 text-slate-400">"Treated as two separate single clicks; no toggle"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Double click after a session switch"</td>
                                <td class="p-4 text-slate-400">"Treated as a fresh single click (previous click stamp is cleared on session change)"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <h3 class="text-base font-semibold text-white mb-2">"DevTools Sub-tab Bar"</h3>
                <p class="text-slate-400 mb-4">
                    "Click "
                    <code class="text-blue-400">"[i] Inspector"</code>
                    " / "
                    <code class="text-blue-400">"[p] Performance"</code>
                    " / "
                    <code class="text-blue-400">"[n] Network"</code>
                    " to switch the active panel. Equivalent to pressing "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"i"</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"p"</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"n"</kbd>
                    " keys."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"Inspector Tree"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800 mb-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Interaction"</th>
                                <th class="p-4 font-medium">"Result"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Click a tree row"</td>
                                <td class="p-4 text-slate-400">"Select it (equivalent to \u{2191}/\u{2193} keyboard navigation)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Click the \u{25b6}/\u{25bc} glyph at the row's left edge"</td>
                                <td class="p-4 text-slate-400">"Expand or collapse the node (equivalent to \u{2192}/\u{2190} keyboard expand/collapse)"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-500 text-sm mb-4">"Both clicks dispatch a layout fetch under the same debounce and cache rules as keyboard navigation."</p>

                <h3 class="text-base font-semibold text-white mb-2">"Performance Frame Chart"</h3>
                <p class="text-slate-400 mb-4">
                    "Click a frame bar in the chart to select it. Equivalent to "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Tab"</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+Tab"</kbd>
                    " in the frames view. Clicking outside any frame bar (e.g. on the budget-line area) is a no-op."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"Network Table"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800 mb-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Interaction"</th>
                                <th class="p-4 font-medium">"Result"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Click a row in the request table"</td>
                                <td class="p-4 text-slate-400">"Select it; details appear in the side panel (or below in narrow mode)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Click detail-tab bar shortcut"</td>
                                <td class="p-4 text-slate-400">"Switch detail tab ("
                                    <code class="text-blue-400">"[g]"</code>
                                    " / "
                                    <code class="text-blue-400">"[h]"</code>
                                    " / "
                                    <code class="text-blue-400">"[q]"</code>
                                    " / "
                                    <code class="text-blue-400">"[s]"</code>
                                    " / "
                                    <code class="text-blue-400">"[t]"</code>
                                    ")"
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 text-sm">
                    <strong class="text-white">"Network filter input exception:"</strong>
                    " When typing in the network filter input, clicks in the table area are suppressed. However, "
                    "clicks on the DevTools sub-tab bar ("
                    <code class="text-blue-400">"[i]"</code>
                    "/"
                    <code class="text-blue-400">"[p]"</code>
                    "/"
                    <code class="text-blue-400">"[n]"</code>
                    ") still work \u{2014} they switch panels AND exit filter input mode, "
                    "preventing a mouse-only user from being trapped in the filter."
                </p>
            </Section>

            // ── Phase 5: Dialogs and Overlays ────────────────────────
            <Section title="Phase 5: Dialogs and Overlay Clicks">
                <p class="text-slate-400 mb-4">
                    "Phase 5 extended the per-frame region registry to cover every remaining clickable surface. "
                    "After Phase 5, every visible UI element that has a keyboard activator also responds to left-click."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"New Session Dialog"</h3>
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2 mb-4">
                    <li>
                        <strong class="text-white">"Click "
                            <code class="text-blue-400">"[1] Connected"</code>
                            " / "
                            <code class="text-blue-400">"[2] Bootable"</code>
                            " tab headers"
                        </strong>
                        " \u{2192} switches the device list to the corresponding tab. Equivalent to "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"1"</kbd>
                        " / "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"2"</kbd>
                        " keyboard shortcuts."
                    </li>
                    <li>
                        <strong class="text-white">"Click a device row"</strong>
                        " \u{2192} selects that device (single click sets the selection). Then click Launch to start the session."
                    </li>
                    <li>
                        <strong class="text-white">"Click a launch-context field"</strong>
                        " ("
                        <code class="text-blue-400">"Configuration"</code>
                        " / "
                        <code class="text-blue-400">"Mode"</code>
                        " / "
                        <code class="text-blue-400">"Flavor"</code>
                        " / "
                        <code class="text-blue-400">"Entry Point"</code>
                        " / "
                        <code class="text-blue-400">"Dart Defines"</code>
                        ") \u{2192} focuses the field and activates it, mirroring the keyboard "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Enter"</kbd>
                        " press on that field."
                    </li>
                    <li>
                        <strong class="text-white">"Click Launch button"</strong>
                        " \u{2192} launches the selected Flutter session."
                    </li>
                    <li>
                        <strong class="text-white">"Inside the fuzzy modal, click a visible result row"</strong>
                        " \u{2192} selects and confirms the entry in a single click (equivalent to "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"\u{2191}"</kbd>
                        "/"
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"\u{2193}"</kbd>
                        " + "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Enter"</kbd>
                        ")."
                    </li>
                    <li>
                        <strong class="text-white">"The dart-defines modal inside NewSessionDialog is keyboard-only in v1."</strong>
                        " No clickable rows are registered for the dart-defines sub-modal."
                    </li>
                </ul>

                <h3 class="text-base font-semibold text-white mb-2">"Confirm Dialog"</h3>
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2 mb-4">
                    <li>
                        <strong class="text-white">"Click "
                            <code class="text-blue-400">"[y] Yes"</code>
                        </strong>
                        " \u{2192} emits the action stored at the Yes button's index in "
                        <code class="text-blue-400">"state.confirm_dialog_state.actions"</code>
                        " (typically "
                        <code class="text-blue-400">"ConfirmQuit"</code>
                        ", but the registry reads from state so all confirm dialogs \u{2014} quit, unsaved-settings, etc. \u{2014} are clickable generically)."
                    </li>
                    <li>
                        <strong class="text-white">"Click "
                            <code class="text-blue-400">"[n] No"</code>
                        </strong>
                        " \u{2192} emits the corresponding No action (typically "
                        <code class="text-blue-400">"CancelQuit"</code>
                        ")."
                    </li>
                    <li>
                        "The clickable rect covers the bracket + label only ("
                        <code class="text-blue-400">"[y] Yes"</code>
                        " / "
                        <code class="text-blue-400">"[n] No"</code>
                        "). Clicks elsewhere on the modal are no-ops."
                    </li>
                </ul>

                <h3 class="text-base font-semibold text-white mb-2">
                    "Tag Filter Overlay (open with "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"T"</kbd>
                    ")"
                </h3>
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2 mb-4">
                    <li>
                        <strong class="text-white">"Click a tag row"</strong>
                        " \u{2192} sets the selected index "
                        <strong class="text-white">"and"</strong>
                        " toggles the tag's visibility in a single click. There is no separate "
                        "\"select then toggle\" two-click flow \u{2014} one click both navigates and toggles."
                    </li>
                    <li>
                        <strong class="text-white">"Click "
                            <code class="text-blue-400">"[a] All"</code>
                        </strong>
                        " \u{2192} shows all tags (equivalent to the "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"a"</kbd>
                        " keyboard shortcut)."
                    </li>
                    <li>
                        <strong class="text-white">"Click "
                            <code class="text-blue-400">"[n] None"</code>
                        </strong>
                        " \u{2192} hides all tags (equivalent to the "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"n"</kbd>
                        " keyboard shortcut)."
                    </li>
                </ul>

                <h3 class="text-base font-semibold text-white mb-2">
                    "Link Highlight Badges (visible after "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+L"</kbd>
                    ")"
                </h3>
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2 mb-4">
                    <li>
                        <strong class="text-white">"Click a badge "
                            <code class="text-blue-400">"[&lt;char&gt;]"</code>
                        </strong>
                        " \u{2192} emits "
                        <code class="text-blue-400">"Message::SelectLink(&lt;char&gt;)"</code>
                        ", following the link associated with that character. Equivalent to pressing the character key."
                    </li>
                    <li>
                        "The clickable rect is exactly the three-cell badge span ("
                        <code class="text-blue-400">"["</code>
                        ", character, "
                        <code class="text-blue-400">"]"</code>
                        "). Clicks on the adjacent link text are not clickable in v1 \u{2014} intentionally narrow "
                        "to prevent accidental activation during scroll gestures."
                    </li>
                </ul>

                <h3 class="text-base font-semibold text-white mb-2">"Settings Panel"</h3>
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2">
                    <li>
                        <strong class="text-white">"Click a tab header"</strong>
                        " ("
                        <code class="text-blue-400">"1. PROJECT"</code>
                        " / "
                        <code class="text-blue-400">"2. USER"</code>
                        " / "
                        <code class="text-blue-400">"3. LAUNCH"</code>
                        " / "
                        <code class="text-blue-400">"4. VSCODE"</code>
                        ") \u{2192} switches to that settings tab. Equivalent to the "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"1"</kbd>
                        "\u{2013}"
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"4"</kbd>
                        " keyboard shortcuts."
                    </li>
                    <li>
                        <strong class="text-white">"Click a setting row"</strong>
                        " \u{2192} selects it (sets "
                        <code class="text-blue-400">"selected_index"</code>
                        "). Single click does not enter edit mode."
                    </li>
                    <li>
                        <strong class="text-white">"Double-click the same row within 400 ms"</strong>
                        " \u{2192} enters edit mode (equivalent to "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Enter"</kbd>
                        "). This mirrors the Phase 4 log-view double-click pattern."
                    </li>
                    <li>
                        <strong class="text-white">"The Settings dart-defines and extra-args sub-modals are keyboard-only in v1."</strong>
                    </li>
                </ul>
            </Section>

            // ── Modal Precedence ─────────────────────────────────────
            <Section title="Modal Precedence and Sub-Modal Gates">
                <p class="text-slate-400 mb-4">
                    "When a modal is open ("
                    <code class="text-blue-400">"NewSessionDialog"</code>
                    ", "
                    <code class="text-blue-400">"ConfirmDialog"</code>
                    ", "
                    <code class="text-blue-400">"TagFilter"</code>
                    ", "
                    <code class="text-blue-400">"FlutterVersion"</code>
                    ", "
                    <code class="text-blue-400">"Settings"</code>
                    ", "
                    <code class="text-blue-400">"LinkHighlight"</code>
                    "), the renderer does not register base-UI click regions "
                    "(header brackets, log-view rows, session tabs) for the underlying surface. Clicks that "
                    "land outside the modal's own rects are silently dropped \u{2014} they do "
                    <strong class="text-white">"not"</strong>
                    " activate the underlying base-UI region. This guarantees, for example, that clicking on "
                    "header "
                    <code class="text-blue-400">"[r]"</code>
                    " while a "
                    <code class="text-blue-400">"ConfirmDialog"</code>
                    " is shown does not fire a hot reload."
                </p>
                <p class="text-slate-400 mb-3">"The z-index convention is:"</p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mb-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"z-index"</th>
                                <th class="p-4 font-medium">"Layer"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"0"</td>
                                <td class="p-4 text-slate-400">"Base UI (header, tabs, log view, DevTools panels)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"1"</td>
                                <td class="p-4 text-slate-400">
                                    "Primary modals ("
                                    <code class="text-blue-400">"NewSessionDialog"</code>
                                    ", "
                                    <code class="text-blue-400">"ConfirmDialog"</code>
                                    ", "
                                    <code class="text-blue-400">"TagFilter"</code>
                                    ", "
                                    <code class="text-blue-400">"FlutterVersion"</code>
                                    ")"
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"2"</td>
                                <td class="p-4 text-slate-400">
                                    "Sub-modals layered atop a primary modal ("
                                    <code class="text-blue-400">"NewSessionDialog"</code>
                                    " fuzzy modal)"
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400">
                    <strong class="text-white">"Sub-modal gates"</strong>
                    " narrow this further for Settings: when a dart-defines or extra-args sub-modal is open inside "
                    <code class="text-blue-400">"Settings"</code>
                    ", "
                    <code class="text-blue-400">"settings::handle_press"</code>
                    " returns "
                    <code class="text-blue-400">"None"</code>
                    " for any click, preventing leaks to the underlying Settings rows. The Settings panel does not "
                    "change "
                    <code class="text-blue-400">"UiMode"</code>
                    " when sub-modals open (they render on top), so the renderer-level modal gate cannot cover them \u{2014} "
                    "the explicit gate inside the Settings press dispatcher closes the gap."
                </p>
            </Section>

            // ── Compact NewSessionDialog ─────────────────────────────
            <Section title="Compact New Session Dialog">
                <p class="text-slate-400">
                    "When the terminal is between 40\u{2013}69 columns wide and 20\u{2013}21 rows tall, the New Session "
                    "Dialog falls back to a compact-vertical layout that does not register device-row click regions. "
                    "In this size range fdemon shows a small hint line (e.g. "
                    <code class="text-blue-400">"Resize for mouse"</code>
                    "); device selection remains fully functional via the keyboard. Resize the terminal wider "
                    "than 70 columns to restore mouse coverage."
                </p>
            </Section>

            // ── Platform Caveats ─────────────────────────────────────
            <Section title="Platform Caveats">
                <h3 class="text-base font-semibold text-white mb-2">"Windows 11 \u{2014} Shift modifier dropped on mouse events"</h3>
                <p class="text-slate-400 mb-2">
                    "Crossterm issue "
                    <a
                        href="https://github.com/crossterm-rs/crossterm/issues/986"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="text-blue-400 hover:underline"
                    >"#986"</a>
                    " documents that Windows 11 (running under modern Windows Terminal or conhost) drops the Shift "
                    "modifier on mouse events before crossterm can read them. The practical impact:"
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mb-4">
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+Wheel"</kbd>
                        " degrades to plain wheel in Normal, LinkHighlight, and DevTools/Network modes"
                    </li>
                    <li>"Page-step scrolling via the wheel is therefore unavailable on Windows 11"</li>
                    <li>
                        "Workaround: use "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"PageUp"</kbd>
                        " / "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"PageDown"</kbd>
                        " keys for page-step navigation \u{2014} these are unaffected by the Shift-drop bug"
                    </li>
                </ul>
                <p class="text-slate-500 text-sm mb-6">"Other platforms (macOS, Linux, older Windows builds) are not affected."</p>

                <h3 class="text-base font-semibold text-white mb-2">"Legacy Windows conhost \u{2014} mouse capture silently ignored"</h3>
                <p class="text-slate-400 mb-6">
                    "If your terminal is the legacy "
                    <code class="text-blue-400">"conhost.exe"</code>
                    " shipped before Windows 10, mouse capture escape sequences are silently ignored and wheel events "
                    "are never delivered to fdemon. Wheel events fall through to the host terminal's scrollback "
                    "(which is often the desired behavior anyway). Set "
                    <code class="text-blue-400">"enable_mouse = false"</code>
                    " in "
                    <code class="text-blue-400">"config.toml"</code>
                    " to opt out cleanly."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"IDE built-in terminals"</h3>
                <p class="text-slate-400 mb-3">
                    "IDE-embedded terminals (Zed, VS Code, JetBrains, Cursor, Windsurf, Fleet, Neovim "
                    <code class="text-blue-400">":terminal"</code>
                    ") wrap the terminal grid in their own event-handling layer that intercepts a subset of keys "
                    "and mouse buttons before the TTY ever sees them. The fix philosophy in this app \u{2014} "
                    "drop "
                    <code class="text-blue-400">"?1003"</code>
                    ", copy via right-click, fall back via "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    ", anchor selection in the scrollback buffer \u{2014} assumes a stand-alone terminal that simply "
                    "forwards the events. IDE terminals violate that assumption in three recurring ways:"
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2 mb-4">
                    <li>
                        <strong class="text-white">"Right-click is swallowed."</strong>
                        " The IDE keeps the right mouse button for its own context menu (or drops it entirely). "
                        "Right-click-to-copy never fires because the event never reaches fdemon."
                    </li>
                    <li>
                        <strong class="text-white">
                            <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+\u{003c}char\u{003e}"</kbd>
                            " chords are eaten."
                        </strong>
                        " The IDE binds Alt-modified keys to its own commands, menu mnemonics, or pane navigation, "
                        "so "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                        " does not toggle capture."
                    </li>
                    <li>
                        <strong class="text-white">"Selection is anchored to viewport coordinates, not buffer rows."</strong>
                        " Standalone terminals (Alacritty, iTerm2, kitty, macOS Terminal, Ghostty, Wezterm, "
                        "Windows Terminal, GNOME Terminal) anchor a Shift+drag selection to the scrollback buffer, "
                        "so the selection tracks the content as new lines arrive. Several IDE terminals anchor to "
                        "viewport coordinates instead \u{2014} when new logs scroll the buffer up, the highlight stays "
                        "at the same screen rows but now covers different log content. Releasing the mouse copies "
                        "the wrong text."
                    </li>
                </ul>

                <h4 class="text-sm font-semibold text-slate-300 mb-3">"Per-IDE summary"</h4>
                <div class="overflow-x-auto mb-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-3 font-medium">"IDE"</th>
                                <th class="p-3 font-medium">"Right-click \u{2192} fdemon"</th>
                                <th class="p-3 font-medium">
                                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+\u{003c}char\u{003e}"</kbd>
                                    " \u{2192} fdemon"
                                </th>
                                <th class="p-3 font-medium">"Selection tracks scrollback"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950 text-slate-400">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"Zed"</td>
                                <td class="p-3">"No"</td>
                                <td class="p-3">"No (intercepted by Zed)"</td>
                                <td class="p-3">"No (viewport-anchored)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"VS Code"</td>
                                <td class="p-3">"No (always intercepted)"</td>
                                <td class="p-3">"Partial (workarounds; some combos broken)"</td>
                                <td class="p-3">"Mostly yes for static scroll; rough during live scroll"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"Cursor"</td>
                                <td class="p-3">"No (inherits VS Code)"</td>
                                <td class="p-3">"Partial (inherits VS Code)"</td>
                                <td class="p-3">"Same as VS Code"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"Windsurf"</td>
                                <td class="p-3">"No (inherits VS Code)"</td>
                                <td class="p-3">"Partial (inherits VS Code)"</td>
                                <td class="p-3">"Same as VS Code"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"JetBrains (IntelliJ / RustRover / CLion / PyCharm)"</td>
                                <td class="p-3">"Configurable, but conflicts with mouse reporting"</td>
                                <td class="p-3">"Partial (Classic engine OK; reworked 2025 engine regressed on macOS)"</td>
                                <td class="p-3">"Not specifically documented"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"Fleet"</td>
                                <td class="p-3">"Likely no"</td>
                                <td class="p-3">"No (open feature request for \"Option as Meta\")"</td>
                                <td class="p-3">"Unknown"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"Neovim :terminal"</td>
                                <td class="p-3">"Inconsistent (left-click better)"</td>
                                <td class="p-3">"Yes by default in terminal-mode"</td>
                                <td class="p-3">"Buffer-anchored, but you must leave terminal-mode to select"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-3 text-white font-medium">"Helix"</td>
                                <td class="p-3">"N/A \u{2014} no embedded terminal"</td>
                                <td class="p-3">"N/A"</td>
                                <td class="p-3">"N/A"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <h4 class="text-sm font-semibold text-slate-300 mb-3">"Per-IDE detail and workarounds"</h4>

                <p class="text-slate-400 mb-3">
                    <strong class="text-white">"Zed."</strong>
                    " All three behaviors are confirmed: right-click is dropped, "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    " never reaches fdemon, and Shift+drag selections drift visually as new log lines arrive. "
                    "There is no user-configurable workaround today. Tracking issues: "
                    <a href="https://github.com/zed-industries/zed/issues/10647" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"zed-industries/zed#10647"</a>
                    " (user-configurable mouse bindings), "
                    <a href="https://github.com/zed-industries/zed/issues/21387" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"#21387"</a>
                    " (Alt forwarding), "
                    <a href="https://github.com/zed-industries/zed/issues/14543" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"#14543"</a>
                    " (Alt/Ctrl shell combos on Linux). For full mouse support, run fdemon in a stand-alone terminal."
                </p>

                <p class="text-slate-400 mb-3">
                    <strong class="text-white">"VS Code."</strong>
                    " Right-click cannot be cleanly forwarded to the terminal app; the closest mitigation is "
                    <code class="text-blue-400">"\"terminal.integrated.rightClickBehavior\": \"paste\""</code>
                    " to suppress the IDE context menu (the right-button-down event is still consumed). "
                    "For Alt forwarding, set "
                    <code class="text-blue-400">"\"terminal.integrated.sendKeybindingsToShell\": true"</code>
                    " and remove conflicting chords via "
                    <code class="text-blue-400">"\"terminal.integrated.commandsToSkipShell\""</code>
                    ". Selection is mostly buffer-anchored but has known rough edges during streaming output ("
                    <a href="https://github.com/xtermjs/xterm.js/issues/5198" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"xtermjs/xterm.js#5198"</a>
                    ", "
                    <a href="https://github.com/microsoft/vscode/issues/142927" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"microsoft/vscode#142927"</a>
                    ")."
                </p>

                <p class="text-slate-400 mb-3">
                    <strong class="text-white">"Cursor / Windsurf."</strong>
                    " Both are VS Code forks and ship the same xterm.js-based terminal layer with no terminal-specific changes. "
                    "Apply the VS Code workarounds above."
                </p>

                <p class="text-slate-400 mb-3">
                    <strong class="text-white">"JetBrains IDEs."</strong>
                    " Enable "
                    <strong class="text-white">"Settings \u{2192} Tools \u{2192} Terminal \u{2192} Mouse Reporting"</strong>
                    " so mouse events are forwarded to fdemon. On macOS, also enable "
                    <strong class="text-white">"\"Use Option as Meta key\""</strong>
                    " so "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    " reaches fdemon \u{2014} this option is reliable on the Classic terminal engine but regressed in the "
                    "Reworked 2025 engine ("
                    <a href="https://youtrack.jetbrains.com/issue/IDEA-165184/Add-Use-Option-as-Meta-key-support-to-terminal" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"IDEA-165184"</a>
                    ", "
                    <a href="https://youtrack.jetbrains.com/issue/IJPL-181613/New-Terminal-Option-as-Meta-key-does-not-work-on-macOS" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"IJPL-181613"</a>
                    "). If you are on macOS and need "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    ", stay on the Classic engine until JetBrains ships the fix. The \"Override IDE shortcuts\" toggle "
                    "helps reduce other Alt collisions but has its own bugs ("
                    <a href="https://youtrack.jetbrains.com/issue/IJPL-107345/Terminal-override-IDE-shortcuts-option-doesnt-work-as-expected" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"IJPL-107345"</a>
                    "). Right-click and selection-during-scroll behavior with mouse reporting active is not cleanly "
                    "documented; treat them as unreliable ("
                    <a href="https://youtrack.jetbrains.com/issue/IDEA-383430/Mouse-markings-interfere-with-mouse-reporting" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"IDEA-383430"</a>
                    ")."
                </p>

                <p class="text-slate-400 mb-3">
                    <strong class="text-white">"Fleet."</strong>
                    " Even less mature than IntelliJ's terminal \u{2014} no \"Option as Meta\" yet ("
                    <a href="https://youtrack.jetbrains.com/issue/FL-24138/Terminal-on-Mac-Option-to-Use-Option-Key-as-Meta-Key" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"FL-24138"</a>
                    "). Use a stand-alone terminal."
                </p>

                <p class="text-slate-400 mb-3">
                    <strong class="text-white">"Neovim :terminal."</strong>
                    " Mouse passthrough requires "
                    <code class="text-blue-400">"set mouse=a"</code>
                    ". Right-click is inconsistently forwarded because Neovim binds it to its own popup "
                    "("
                    <code class="text-blue-400">"mousemodel=popup"</code>
                    "); see "
                    <a href="https://github.com/neovim/neovim/issues/3669" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"neovim/neovim#3669"</a>
                    " and "
                    <a href="https://github.com/neovim/neovim/issues/23875" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"#23875"</a>
                    ". "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+\u{003c}char\u{003e}"</kbd>
                    " chords generally pass through in terminal-mode unless you have rebound them. Selection is "
                    "buffer-anchored, but you must leave terminal-mode (press "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Esc"</kbd>
                    ") to enter visual selection \u{2014} which breaks the live \"select while logs stream\" workflow."
                </p>

                <p class="text-slate-400 mb-4">
                    <strong class="text-white">"Helix."</strong>
                    " No built-in terminal; pair Helix with "
                    <code class="text-blue-400">"tmux"</code>
                    " or "
                    <code class="text-blue-400">"zellij"</code>
                    " and run fdemon in a stand-alone terminal pane."
                </p>

                <div class="bg-slate-900/50 border border-blue-500/30 rounded-lg p-4 mb-6">
                    <p class="text-slate-300 font-semibold mb-1">"Recommendation"</p>
                    <p class="text-slate-400">
                        "If your IDE's terminal eats any of right-click, "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                        ", or buffer-anchored selection, the simplest path is to run fdemon in a stand-alone terminal emulator. "
                        "Pause log streaming ("
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Space"</kbd>
                        ") before Shift+drag if you must select inside an IDE terminal \u{2014} frozen content can't drift "
                        "under your selection."
                    </p>
                </div>

                <h3 class="text-base font-semibold text-white mb-2">"Pointer shape (OSC 22)"</h3>
                <p class="text-slate-400 mb-3">
                    "While the TUI is active, fdemon requests the "
                    <code class="text-blue-400">"default"</code>
                    " (arrow) OS-level pointer shape via the OSC 22 escape sequence, and resets it on exit. "
                    "This keeps the cursor from staying as a text I-beam while hovering over buttons and clickable regions."
                </p>
                <p class="text-slate-400 mb-3">"OSC 22 support is best-effort and depends on your terminal emulator:"</p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mb-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Terminal"</th>
                                <th class="p-4 font-medium">"OSC 22 Support"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"kitty"</td>
                                <td class="p-4 text-slate-400">"Supported"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Ghostty"</td>
                                <td class="p-4 text-slate-400">"Supported"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Foot"</td>
                                <td class="p-4 text-slate-400">"Supported"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"xterm"</td>
                                <td class="p-4 text-slate-400">"Supported"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Alacritty"</td>
                                <td class="p-4 text-slate-400">
                                    "Requires "
                                    <code class="text-blue-400">"terminal.osc22 = true"</code>
                                    " in Alacritty config"
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"iTerm2"</td>
                                <td class="p-4 text-slate-400">"Silently ignored \u{2014} I-beam remains"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"macOS Terminal.app"</td>
                                <td class="p-4 text-slate-400">"Silently ignored \u{2014} I-beam remains"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Windows Terminal"</td>
                                <td class="p-4 text-slate-400">"Silently ignored \u{2014} I-beam remains"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"GNOME Terminal"</td>
                                <td class="p-4 text-slate-400">"Silently ignored \u{2014} I-beam remains"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 mb-2">
                    "Terminals that do not support OSC 22 silently ignore the escape sequence; there is no "
                    "functional regression, only a cosmetic one (the pointer shape stays as an I-beam)."
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2">
                    <li>
                        "Pointer-shapes reference: "
                        <a href="https://sw.kovidgoyal.net/kitty/pointer-shapes/" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"https://sw.kovidgoyal.net/kitty/pointer-shapes/"</a>
                    </li>
                    <li>
                        "Terminal compatibility table: "
                        <a href="https://can-i-use-terminal.github.io/features/osc22.html" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"https://can-i-use-terminal.github.io/features/osc22.html"</a>
                    </li>
                </ul>
            </Section>

            // ── Runtime Toggle ───────────────────────────────────────
            <Section title="Runtime Toggle">
                <p class="text-slate-400 mb-3">
                    "Press "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    " in any mode to toggle mouse capture on or off without restarting fdemon. "
                    "The "
                    <code class="text-blue-400">"[mouse]"</code>
                    " / "
                    <code class="text-blue-400">"[mouse-off]"</code>
                    " badge in the status bar reflects the current state."
                </p>
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2 mb-4">
                    <li>
                        <strong class="text-white">
                            <code class="text-blue-400">"[mouse]"</code>
                        </strong>
                        " \u{2014} capture is active; wheel scroll, clicks, and right-click-copy all work."
                    </li>
                    <li>
                        <strong class="text-white">
                            <code class="text-blue-400">"[mouse-off]"</code>
                        </strong>
                        " \u{2014} capture is suspended; native terminal selection and scrollback work unimpeded."
                    </li>
                </ul>
                <p class="text-slate-400">
                    "The toggle is in-process only. It does not write to "
                    <code class="text-blue-400">"config.toml"</code>
                    "; restart returns to the value of "
                    <code class="text-blue-400">"[ui] enable_mouse"</code>
                    ". Use the toggle for ad-hoc suspends; use the config setting for a permanent opt-out."
                </p>
            </Section>

            // ── Disabling Mouse Capture ──────────────────────────────
            <Section title="Disabling Mouse Capture">
                <p class="text-slate-400 mb-4">
                    "For a permanent opt-out \u{2014} legacy Windows conhost, terminals without Shift+drag support, "
                    "or a preference for native wheel scrollback \u{2014} disable mouse capture in your config:"
                </p>
                <CodeBlock language="toml" code="[ui]\nenable_mouse = false" />
                <p class="text-slate-400 mt-4">
                    "Restart fdemon after changing this setting. While disabled, "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+m"</kbd>
                    " has no effect (capture is already off). See the "
                    <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>
                    " page for the full setting reference, including the \"When to disable mouse capture\" callout."
                </p>
            </Section>

            // ── Future Work ──────────────────────────────────────────
            <Section title="Future Work">
                <ul class="list-disc list-inside text-slate-400 space-y-2 ml-2">
                    <li>"Drag-to-resize panel splits."</li>
                    <li>"Hover tooltips."</li>
                    <li>"Project-selector mouse support."</li>
                    <li>
                        "Right-click context menus (right-click currently has a fixed action on log rows \u{2014} full line copy; "
                        "a multi-item context menu is deferred until a concrete use case arrives)."
                    </li>
                    <li>"Horizontal-scroll consumers (log timeline panning, DevTools secondary-axis navigation)."</li>
                </ul>
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
fn ScrollRow(mode: &'static str, plain: &'static str, shift: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50 transition-colors">
            <td class="p-4 text-white font-medium">{mode}</td>
            <td class="p-4 text-slate-400">{plain}</td>
            <td class="p-4 text-slate-500 hidden md:table-cell">{shift}</td>
        </tr>
    }
}
