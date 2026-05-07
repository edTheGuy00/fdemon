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
                " (the default). This page describes scroll routing, click semantics for each UI surface, "
                "modal precedence, and platform caveats. For the on/off setting, see "
                <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>"."
            </p>

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
                        " → page up / page down"
                    </li>
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Shift+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Shift+Wheel"</kbd>
                        " → "
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
                    "DevTools — Inspector"
                    <span class="text-slate-500 font-normal text-sm ml-2">"(special case)"</span>
                </h3>
                <ul class="list-disc list-inside text-slate-400 space-y-1 ml-2 mb-4">
                    <li>
                        "Plain wheel, "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Shift+Wheel"</kbd>
                        " → single-step tree row navigation (no page-step analogue)"
                    </li>
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Wheel"</kbd>
                        " alone, "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Wheel"</kbd>
                        " alone → no-op"
                    </li>
                    <li>
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Ctrl+Shift+Wheel"</kbd>
                        ", "
                        <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Alt+Shift+Wheel"</kbd>
                        " → single-step navigation (Shift is prioritized over the no-op Ctrl/Alt rule)"
                    </li>
                </ul>

                <h3 class="text-base font-semibold text-white mb-2">"Horizontal Scroll"</h3>
                <p class="text-slate-400">
                    "Left and right scroll events (touchpad horizontal scroll) are "
                    <strong class="text-white">"no-ops in all modes"</strong>
                    ". Future phases may map horizontal scroll to log timeline panning or a DevTools secondary-axis navigation."
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
                                <td class="p-4 text-slate-400">"Toggles the entry’s stack trace expansion (if it has a stack trace)"</td>
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
                                <td class="p-4 text-slate-400">"Select it (equivalent to ↑/↓ keyboard navigation)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Click the ▶/▼ glyph at the row’s left edge"</td>
                                <td class="p-4 text-slate-400">"Expand or collapse the node (equivalent to →/← keyboard expand/collapse)"</td>
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
                    ") still work — they switch panels AND exit filter input mode, "
                    "preventing a mouse-only user from being trapped in the filter."
                </p>
            </Section>

            // ── Phase 5: Dialogs and Overlays ────────────────────────
            <Section title="Phase 5: Dialogs and Overlay Clicks">
                <h3 class="text-base font-semibold text-white mb-2">"New Session Dialog"</h3>
                <div class="overflow-hidden rounded-lg border border-slate-800 mb-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Surface"</th>
                                <th class="p-4 font-medium">"Interaction"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Result"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Tab bar"</td>
                                <td class="p-4 text-slate-400">"Click a tab label"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Switch to that pane (Device / Launch Context)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Device list"</td>
                                <td class="p-4 text-slate-400">"Click a device row"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Select that device"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Launch Context fields"</td>
                                <td class="p-4 text-slate-400">"Click a field row"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Focus that field for editing"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50 transition-colors">
                                <td class="p-4 text-white font-medium">"Launch button"</td>
                                <td class="p-4 text-slate-400">"Click “Launch”"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Start the Flutter session (equivalent to Enter)"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <h3 class="text-base font-semibold text-white mb-2">"Confirm Dialog"</h3>
                <p class="text-slate-400 mb-4">
                    "The confirmation dialog (shown when quitting with running sessions, or closing a session) renders "
                    <code class="text-blue-400">"[ Yes ]"</code>
                    " and "
                    <code class="text-blue-400">"[ No ]"</code>
                    " as clickable regions. Clicking either button fires the same action as pressing "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"y"</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"n"</kbd>
                    " or "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Enter"</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Esc"</kbd>
                    "."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"Tag Filter Overlay"</h3>
                <p class="text-slate-400 mb-4">
                    "Clicking a tag row in the filter overlay toggles that tag’s visibility, equivalent to pressing "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Space"</kbd>
                    " on the highlighted row. The overlay closes on click-outside (any click outside the overlay bounds)."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"Link Highlight Badges"</h3>
                <p class="text-slate-400 mb-4">
                    "In Link Highlight mode, each highlighted URL or file path is accompanied by a numbered badge "
                    "(e.g. "
                    <code class="text-blue-400">"[1]"</code>
                    ", "
                    <code class="text-blue-400">"[2]"</code>
                    "). Clicking a badge opens the corresponding link in the configured editor or browser, "
                    "the same as pressing the corresponding number key."
                </p>

                <h3 class="text-base font-semibold text-white mb-2">"Settings Panel Rows"</h3>
                <p class="text-slate-400">
                    "Each row in the Settings panel is clickable. Clicking a row selects it "
                    "(equivalent to navigating with "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"↑"</kbd>
                    " / "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"↓"</kbd>
                    ") and opens the inline editor or sub-modal for that setting "
                    "(equivalent to pressing "
                    <kbd class="font-mono text-xs bg-slate-800 px-1 py-0.5 rounded">"Enter"</kbd>
                    ")."
                </p>
            </Section>

            // ── Modal Precedence ─────────────────────────────────────
            <Section title="Modal Precedence and Sub-Modal Gates">
                <p class="text-slate-400">
                    "Flutter Demon uses a layered modal system. When a dialog or overlay is open, "
                    "clicks on the base UI surfaces (log view, DevTools panels, header shortcuts) are suppressed "
                    "at the renderer level — only the topmost modal layer receives click events. "
                    "The precedence order from highest to lowest is:"
                </p>
                <ol class="list-decimal list-inside text-slate-400 space-y-1 ml-2 mt-2">
                    <li><strong class="text-white">"Confirm dialog"</strong>" — blocks all other input when open"</li>
                    <li><strong class="text-white">"Tag filter overlay"</strong></li>
                    <li><strong class="text-white">"New Session dialog"</strong>" sub-modals (fuzzy picker, dart-defines modal, extra-args modal)"</li>
                    <li><strong class="text-white">"New Session dialog"</strong>" (main panes)"</li>
                    <li><strong class="text-white">"Settings panel"</strong>" sub-modals (dart-defines, extra-args)"</li>
                    <li><strong class="text-white">"Settings panel"</strong></li>
                    <li><strong class="text-white">"Base UI"</strong>" (Normal, DevTools, etc.)"</li>
                </ol>
                <p class="text-slate-400 mt-3">
                    "Sub-modal gates: inside the New Session dialog, clicks on the main pane rows are suppressed "
                    "while a sub-modal (e.g. the fuzzy picker) is open. Only the sub-modal’s own regions are active."
                </p>
            </Section>

            // ── Compact NewSessionDialog ─────────────────────────────
            <Section title="Compact New Session Dialog">
                <p class="text-slate-400">
                    "When the terminal is narrower than the minimum width threshold for the full New Session dialog layout, "
                    "the dialog renders in a compact single-column mode. In this mode, the tab bar is hidden and the "
                    "device list and launch-context fields are stacked vertically. Mouse click regions are recalculated "
                    "for the compact layout — all clickable surfaces remain functional, but their positions differ "
                    "from the two-column layout."
                </p>
                <p class="text-slate-400 mt-3">
                    "If you find that clicks are registering on unexpected rows, resize your terminal wider to use the "
                    "full layout, which has more generous hit targets."
                </p>
            </Section>

            // ── Platform Caveats ─────────────────────────────────────
            <Section title="Platform Caveats">
                <h3 class="text-base font-semibold text-white mb-2">"Windows 11 — Shift modifier dropped on mouse events"</h3>
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
                        " keys for page-step navigation — these are unaffected by the Shift-drop bug"
                    </li>
                </ul>
                <p class="text-slate-500 text-sm mb-6">"Other platforms (macOS, Linux, older Windows builds) are not affected."</p>

                <h3 class="text-base font-semibold text-white mb-2">"Legacy Windows conhost — mouse capture silently ignored"</h3>
                <p class="text-slate-400">
                    "If your terminal is the legacy "
                    <code class="text-blue-400">"conhost.exe"</code>
                    " shipped before Windows 10, mouse capture escape sequences are silently ignored and wheel events "
                    "are never delivered to fdemon. Wheel events fall through to the host terminal’s scrollback "
                    "(which is often the desired behavior anyway). Set "
                    <code class="text-blue-400">"enable_mouse = false"</code>
                    " in "
                    <code class="text-blue-400">"config.toml"</code>
                    " to opt out cleanly."
                </p>
            </Section>

            // ── Disabling Mouse Capture ──────────────────────────────
            <Section title="Disabling Mouse Capture">
                <p class="text-slate-400 mb-4">
                    "If you prefer wheel events to drive your terminal’s native scrollback, or if you are on "
                    "legacy Windows conhost, disable mouse capture in your project’s "
                    <code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>
                    ":"
                </p>
                <CodeBlock language="toml" code="[ui]\nenable_mouse = false" />
                <p class="text-slate-400 mt-4">
                    "Restart fdemon after changing this setting. See the "
                    <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>
                    " page for the full setting reference, including the “When to disable mouse capture” callout."
                </p>
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
