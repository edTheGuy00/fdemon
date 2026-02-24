use leptos::prelude::*;

use crate::components::icons::{Cpu, Layout, Smartphone, Zap};

// ── Changelog types ───────────────────────────────────────────────────────────

pub struct ChangelogChange {
    pub description: &'static str,
    pub scope: Option<&'static str>,
}

pub struct ChangelogGroup {
    pub group: &'static str,
    pub changes: Vec<ChangelogChange>,
}

pub struct ChangelogEntry {
    pub version: &'static str,
    pub date: &'static str,
    pub groups: Vec<ChangelogGroup>,
}

pub fn changelog_entries() -> Vec<ChangelogEntry> {
    vec![
        ChangelogEntry {
            version: "0.1.0",
            date: "2026-02-24",
            groups: vec![
                ChangelogGroup {
                    group: "Features",
                    changes: vec![
                        ChangelogChange { description: "Phase 3 — version flag, title bar version, release workflow & install script", scope: None },
                        ChangelogChange { description: "DevTools v2 phase 5 — polish, config, filter input, review & fix plan", scope: None },
                        ChangelogChange { description: "DevTools v2 phase 4 — network monitor tab", scope: None },
                        ChangelogChange { description: "DevTools v2 phase 3 — performance tab overhaul", scope: None },
                        ChangelogChange { description: "DevTools v2 phase 2 — merge Inspector and Layout into unified tab", scope: None },
                        ChangelogChange { description: "DevTools phase 5 — config expansion, connection UI, error UX, performance polish, docs & website", scope: None },
                        ChangelogChange { description: "DevTools phase 4 — TUI panels, key handlers, and review fix plans", scope: None },
                        ChangelogChange { description: "DevTools phase 3 — performance & memory monitoring data pipeline", scope: None },
                        ChangelogChange { description: "Add VM Service client foundation with structured errors and hybrid logging (Phase 1)", scope: None },
                        ChangelogChange { description: "Implement full-screen settings panel with tabbed UI", scope: Some("settings") },
                        ChangelogChange { description: "Complete Phase 5 with startup dialog, config priority, and bugfixes", scope: Some("startup") },
                        ChangelogChange { description: "Implement phase 8 integration & cleanup", scope: Some("new-session-dialog") },
                        ChangelogChange { description: "Implement Phase 1 mock daemon testing infrastructure", scope: Some("e2e") },
                        ChangelogChange { description: "Implement double-'q' quick quit feature", scope: Some("keys") },
                        ChangelogChange { description: "Add Link Highlight Mode for opening files from logs (Phase 3.1)", scope: None },
                        ChangelogChange { description: "Complete Phase 1 - Log filtering, search, and error navigation", scope: None },
                        ChangelogChange { description: "Complete Phase 2 - Error highlighting, stack traces, and horizontal scroll", scope: None },
                        ChangelogChange { description: "Add log view word wrap mode with correct scroll bounds", scope: None },
                        ChangelogChange { description: "Settings launch tab modals — dart defines editor & extra args picker", scope: None },
                        ChangelogChange { description: "Create website", scope: None },
                    ],
                },
                ChangelogGroup {
                    group: "Bug Fixes",
                    changes: vec![
                        ChangelogChange { description: "Phase 2 review remediation — 6 fixes across settings modals", scope: None },
                        ChangelogChange { description: "Resolve Phase 2 bugs — response routing, shutdown, exit handling, selector UI", scope: None },
                        ChangelogChange { description: "Devtools phase 4 bugs — layout tab, refresh, narrow window, browser URL, key nav", scope: None },
                        ChangelogChange { description: "Devtools v2 phase 3 review fixes — 7 tasks + allocation table bug", scope: None },
                        ChangelogChange { description: "Devtools v2 phase 4 review fixes — 7 tasks across 4 crates", scope: None },
                        ChangelogChange { description: "Devtools v2 phase 5 review fixes — 6 tasks across 4 crates", scope: None },
                        ChangelogChange { description: "Implement boolean toggle handler", scope: Some("settings") },
                        ChangelogChange { description: "Address phase 2 review issues — split extensions, refactor ownership, harden error handling", scope: None },
                        ChangelogChange { description: "Widget inspector groupName bug + timeout improvements for large projects", scope: None },
                    ],
                },
                ChangelogGroup {
                    group: "Refactoring",
                    changes: vec![
                        ChangelogChange { description: "Devtools v2 phase 1 — decompose oversized widget and handler files", scope: None },
                        ChangelogChange { description: "Split tui/mod.rs into focused modules", scope: None },
                        ChangelogChange { description: "Split handler.rs into focused modules", scope: None },
                        ChangelogChange { description: "Split log_view.rs into module directory", scope: Some("tui") },
                        ChangelogChange { description: "Implement phase 6.1 file splitting", scope: Some("new-session-dialog") },
                    ],
                },
                ChangelogGroup {
                    group: "Documentation",
                    changes: vec![
                        ChangelogChange { description: "Add log filtering, search, and error navigation to README", scope: None },
                        ChangelogChange { description: "Clean up ARCHITECTURE.md and move code samples to CODE_STANDARDS.md", scope: None },
                        ChangelogChange { description: "Plan phase 1 and phase 2 task breakdowns for cyber-glass redesign", scope: None },
                    ],
                },
            ],
        },
    ]
}

pub struct Feature {
    pub icon: fn() -> AnyView,
    pub title: &'static str,
    pub desc: &'static str,
}

pub fn features() -> Vec<Feature> {
    vec![
        Feature {
            icon: || view! { <Cpu class="w-6 h-6 text-blue-400" /> }.into_any(),
            title: "Blazingly Fast",
            desc: "Written in Rust for instant startup times, minimal memory usage, and zero lag even with large logs.",
        },
        Feature {
            icon: || view! { <Smartphone class="w-6 h-6 text-green-400" /> }.into_any(),
            title: "Multi-Device",
            desc: "Run up to 9 simultaneous sessions. Debug iOS, Android, and Web at the same time from one terminal.",
        },
        Feature {
            icon: || view! { <Zap class="w-6 h-6 text-yellow-400" /> }.into_any(),
            title: "Auto Hot Reload",
            desc: "Smart file watcher monitors your lib/ directory and triggers reload on save with intelligent debouncing.",
        },
        Feature {
            icon: || view! { <Layout class="w-6 h-6 text-purple-400" /> }.into_any(),
            title: "Beautiful TUI",
            desc: "Built with Ratatui. Features scrollable logs, syntax highlighting, session tabs, and vim-style navigation.",
        },
    ]
}

pub struct Keybinding {
    pub key: &'static str,
    pub action: &'static str,
    pub description: &'static str,
}

pub struct KeybindingSection {
    pub title: &'static str,
    pub color: &'static str,
    pub key_color: &'static str,
    pub bindings: Vec<Keybinding>,
}

pub fn all_keybinding_sections() -> Vec<KeybindingSection> {
    vec![
        // ── Normal Mode ──────────────────────────────────────────────
        KeybindingSection {
            title: "General Controls",
            color: "bg-blue-500",
            key_color: "text-blue-400",
            bindings: vec![
                Keybinding { key: "q", action: "Quit", description: "Request to quit (may show confirmation dialog if sessions are running)" },
                Keybinding { key: "qq", action: "Quick Quit", description: "Second q confirms the quit dialog" },
                Keybinding { key: "Esc", action: "Quit", description: "Same as q" },
                Keybinding { key: "Ctrl+C", action: "Force Quit", description: "Emergency exit, bypasses confirmation dialog" },
                Keybinding { key: "c", action: "Clear Logs", description: "Clear all logs in the current session" },
            ],
        },
        KeybindingSection {
            title: "Session Management",
            color: "bg-blue-500",
            key_color: "text-blue-400",
            bindings: vec![
                Keybinding { key: "1-9", action: "Switch Session", description: "Switch to session 1-9 by index" },
                Keybinding { key: "Tab", action: "Next Session", description: "Cycle to the next session" },
                Keybinding { key: "Shift+Tab", action: "Previous Session", description: "Cycle to the previous session" },
                Keybinding { key: "x", action: "Close Session", description: "Close the current session" },
                Keybinding { key: "Ctrl+W", action: "Close Session", description: "Alternative binding to close current session" },
                Keybinding { key: "+", action: "Start New Session", description: "Open New Session Dialog to configure and launch a session" },
            ],
        },
        KeybindingSection {
            title: "App Control",
            color: "bg-blue-500",
            key_color: "text-blue-400",
            bindings: vec![
                Keybinding { key: "r", action: "Hot Reload", description: "Trigger a hot reload (disabled when busy)" },
                Keybinding { key: "R", action: "Hot Restart", description: "Trigger a hot restart (disabled when busy)" },
                Keybinding { key: "s", action: "Stop App", description: "Stop the running app (disabled when busy)" },
            ],
        },
        KeybindingSection {
            title: "Log Navigation",
            color: "bg-blue-500",
            key_color: "text-blue-400",
            bindings: vec![
                Keybinding { key: "j / \u{2193}", action: "Scroll Down", description: "Move down one line" },
                Keybinding { key: "k / \u{2191}", action: "Scroll Up", description: "Move up one line" },
                Keybinding { key: "g", action: "Go to Top", description: "Jump to the beginning of logs" },
                Keybinding { key: "G", action: "Go to Bottom", description: "Jump to the end of logs" },
                Keybinding { key: "Home / End", action: "Top / Bottom", description: "Alternative bindings for top/bottom" },
                Keybinding { key: "PgUp / PgDn", action: "Page Scroll", description: "Scroll up/down one page" },
                Keybinding { key: "h / \u{2190}", action: "Scroll Left", description: "Move left 10 characters" },
                Keybinding { key: "l / \u{2192}", action: "Scroll Right", description: "Move right 10 characters" },
                Keybinding { key: "0", action: "Line Start", description: "Jump to the start of the line" },
                Keybinding { key: "$", action: "Line End", description: "Jump to the end of the line" },
            ],
        },
        KeybindingSection {
            title: "Log Filtering",
            color: "bg-blue-500",
            key_color: "text-blue-400",
            bindings: vec![
                Keybinding { key: "f", action: "Cycle Level Filter", description: "Cycle through: All \u{2192} Errors \u{2192} Warnings \u{2192} Info \u{2192} Debug" },
                Keybinding { key: "F", action: "Cycle Source Filter", description: "Cycle through: All \u{2192} App \u{2192} Daemon \u{2192} Flutter \u{2192} Watcher" },
                Keybinding { key: "Ctrl+F", action: "Reset Filters", description: "Clear all active filters" },
            ],
        },
        KeybindingSection {
            title: "Log Search & Error Navigation",
            color: "bg-blue-500",
            key_color: "text-blue-400",
            bindings: vec![
                Keybinding { key: "/", action: "Start Search", description: "Enter search input mode to type a query" },
                Keybinding { key: "n", action: "Next Match", description: "Jump to the next search match" },
                Keybinding { key: "N", action: "Previous Match", description: "Jump to the previous search match" },
                Keybinding { key: "e", action: "Next Error", description: "Jump to the next error log entry" },
                Keybinding { key: "E", action: "Previous Error", description: "Jump to the previous error log entry" },
                Keybinding { key: "Enter", action: "Toggle Stack Trace", description: "Expand/collapse stack trace of the focused entry" },
                Keybinding { key: "L", action: "Enter Link Mode", description: "Highlight all file references with shortcut badges" },
            ],
        },

        // ── DevTools Mode ─────────────────────────────────────────────
        KeybindingSection {
            title: "DevTools — Panel Navigation",
            color: "bg-cyan-500",
            key_color: "text-cyan-400",
            bindings: vec![
                Keybinding { key: "d", action: "Enter DevTools", description: "Enter DevTools mode (requires VM Service connection)" },
                Keybinding { key: "Esc", action: "Exit DevTools", description: "Return to Normal mode (log view)" },
                Keybinding { key: "i", action: "Inspector Panel", description: "Switch to Widget Inspector panel" },
                Keybinding { key: "p", action: "Performance Panel", description: "Switch to Performance monitoring panel" },
                Keybinding { key: "n", action: "Network Panel", description: "Switch to Network monitoring panel" },
                Keybinding { key: "b", action: "Browser DevTools", description: "Open Flutter DevTools in system browser" },
                Keybinding { key: "q", action: "Quit", description: "Quit the application" },
            ],
        },
        KeybindingSection {
            title: "DevTools — Debug Overlays",
            color: "bg-cyan-500",
            key_color: "text-cyan-400",
            bindings: vec![
                Keybinding { key: "Ctrl+r", action: "Repaint Rainbow", description: "Toggle repaint rainbow overlay on device" },
                Keybinding { key: "Ctrl+p", action: "Performance Overlay", description: "Toggle performance overlay on device" },
                Keybinding { key: "Ctrl+d", action: "Debug Paint", description: "Toggle debug paint overlay on device" },
            ],
        },
        KeybindingSection {
            title: "DevTools — Widget Inspector",
            color: "bg-cyan-500",
            key_color: "text-cyan-400",
            bindings: vec![
                Keybinding { key: "\u{2191} / k", action: "Move Up", description: "Move selection up in widget tree" },
                Keybinding { key: "\u{2193} / j", action: "Move Down", description: "Move selection down in widget tree" },
                Keybinding { key: "\u{2192} / Enter", action: "Expand", description: "Expand selected tree node" },
                Keybinding { key: "\u{2190} / h", action: "Collapse", description: "Collapse selected tree node" },
                Keybinding { key: "r", action: "Refresh", description: "Refresh widget tree from VM Service" },
            ],
        },
        KeybindingSection {
            title: "DevTools — Performance Monitor",
            color: "bg-cyan-500",
            key_color: "text-cyan-400",
            bindings: vec![
                Keybinding { key: "s", action: "Toggle Allocation Sort", description: "Toggle allocation table sort between BySize and ByInstances" },
                Keybinding { key: "\u{2190}", action: "Previous Frame", description: "Select the previous frame in the bar chart" },
                Keybinding { key: "\u{2192}", action: "Next Frame", description: "Select the next frame in the bar chart" },
                Keybinding { key: "Esc", action: "Deselect / Exit", description: "Deselect current frame, or exit DevTools if no frame selected" },
            ],
        },
        KeybindingSection {
            title: "DevTools — Network Monitor",
            color: "bg-cyan-500",
            key_color: "text-cyan-400",
            bindings: vec![
                Keybinding { key: "j / \u{2193}", action: "Navigate Down", description: "Move to next request in the list" },
                Keybinding { key: "k / \u{2191}", action: "Navigate Up", description: "Move to previous request in the list" },
                Keybinding { key: "PgDn", action: "Page Down", description: "Skip forward 10 requests" },
                Keybinding { key: "PgUp", action: "Page Up", description: "Skip back 10 requests" },
                Keybinding { key: "Enter", action: "Select Request", description: "Open request detail view for the selected request" },
                Keybinding { key: "Esc", action: "Deselect / Exit", description: "Deselect current request, or exit DevTools if nothing selected" },
                Keybinding { key: "g", action: "General Tab", description: "Switch detail view to General tab" },
                Keybinding { key: "h", action: "Headers Tab", description: "Switch detail view to Headers tab" },
                Keybinding { key: "q", action: "Request Body Tab", description: "Switch detail view to Request Body tab" },
                Keybinding { key: "s", action: "Response Body Tab", description: "Switch detail view to Response Body tab" },
                Keybinding { key: "t", action: "Timing Tab", description: "Switch detail view to Timing tab" },
                Keybinding { key: "Space", action: "Toggle Recording", description: "Start or stop recording network requests" },
                Keybinding { key: "Ctrl+X", action: "Clear History", description: "Clear all recorded network requests" },
                Keybinding { key: "/", action: "Enter Filter Mode", description: "Enter filter input mode to type a filter query" },
            ],
        },
        KeybindingSection {
            title: "Network Filter Input",
            color: "bg-cyan-500",
            key_color: "text-cyan-400",
            bindings: vec![
                Keybinding { key: "Type", action: "Filter Input", description: "Add character to filter query" },
                Keybinding { key: "Backspace", action: "Delete Character", description: "Remove last character from filter query" },
                Keybinding { key: "Enter", action: "Apply Filter", description: "Apply the filter and exit filter input mode" },
                Keybinding { key: "Esc", action: "Cancel Filter", description: "Discard filter changes and exit filter input mode" },
            ],
        },

        // ── New Session Dialog ───────────────────────────────────────
        KeybindingSection {
            title: "New Session Dialog",
            color: "bg-green-500",
            key_color: "text-green-400",
            bindings: vec![
                Keybinding { key: "Tab", action: "Switch Pane", description: "Switch focus between Target Selector and Launch Context" },
                Keybinding { key: "1 / 2", action: "Device Tabs", description: "Switch to Connected (1) or Bootable (2) devices tab" },
                Keybinding { key: "\u{2191} / k", action: "Navigate Up", description: "Move up in device list or previous field" },
                Keybinding { key: "\u{2193} / j", action: "Navigate Down", description: "Move down in device list or next field" },
                Keybinding { key: "Enter", action: "Select / Launch", description: "Select device, activate field, or launch session" },
                Keybinding { key: "\u{2190} / \u{2192}", action: "Change Mode", description: "Cycle mode (when Mode field focused)" },
                Keybinding { key: "r", action: "Refresh", description: "Refresh device list" },
                Keybinding { key: "Esc", action: "Close", description: "Close modal or dialog" },
            ],
        },

        // ── Fuzzy Search Modal ───────────────────────────────────────
        KeybindingSection {
            title: "Fuzzy Search Modal",
            color: "bg-green-500",
            key_color: "text-green-400",
            bindings: vec![
                Keybinding { key: "Type", action: "Filter / Input", description: "Filter existing items or enter custom value" },
                Keybinding { key: "\u{2191} / \u{2193}", action: "Navigate", description: "Navigate through filtered results" },
                Keybinding { key: "Enter", action: "Confirm", description: "Select highlighted item or use custom text" },
                Keybinding { key: "Backspace", action: "Delete Char", description: "Delete last character from query" },
                Keybinding { key: "Esc", action: "Cancel", description: "Close modal without changes" },
            ],
        },

        // ── Dart Defines Modal ───────────────────────────────────────
        KeybindingSection {
            title: "Dart Defines Modal",
            color: "bg-green-500",
            key_color: "text-green-400",
            bindings: vec![
                Keybinding { key: "Tab", action: "Switch Pane", description: "Switch between List and Edit panes" },
                Keybinding { key: "\u{2191} / \u{2193}", action: "Navigate", description: "Navigate items in list pane" },
                Keybinding { key: "Enter", action: "Action", description: "Load item (List) / Save or Delete (Edit)" },
                Keybinding { key: "Esc", action: "Save & Close", description: "Save all changes and close modal" },
            ],
        },

        // ── Search Input Mode ────────────────────────────────────────
        KeybindingSection {
            title: "Search Input Mode",
            color: "bg-cyan-500",
            key_color: "text-cyan-400",
            bindings: vec![
                Keybinding { key: "Type", action: "Input Character", description: "Add character to the search query" },
                Keybinding { key: "Backspace", action: "Delete Character", description: "Remove the last character from the query" },
                Keybinding { key: "Ctrl+U", action: "Clear Input", description: "Clear the entire search query" },
                Keybinding { key: "Enter", action: "Submit Search", description: "Exit search input mode, keep query active" },
                Keybinding { key: "Esc", action: "Cancel Search", description: "Exit search input mode, keep current query" },
            ],
        },

        // ── Link Highlight Mode ──────────────────────────────────────
        KeybindingSection {
            title: "Link Highlight Mode",
            color: "bg-orange-500",
            key_color: "text-orange-400",
            bindings: vec![
                Keybinding { key: "1-9", action: "Open Link", description: "Open the file reference labeled 1-9" },
                Keybinding { key: "a-z", action: "Open Link", description: "Open the file reference labeled 10-35" },
                Keybinding { key: "j / k", action: "Scroll", description: "Scroll down/up while in link mode" },
                Keybinding { key: "PgUp / PgDn", action: "Page Scroll", description: "Scroll up/down one page" },
                Keybinding { key: "Esc / L", action: "Exit Link Mode", description: "Return to normal mode" },
            ],
        },

        // ── Settings Panel ───────────────────────────────────────────
        KeybindingSection {
            title: "Settings Panel",
            color: "bg-purple-500",
            key_color: "text-purple-400",
            bindings: vec![
                Keybinding { key: ",", action: "Open Settings", description: "Open the full-screen settings panel" },
                Keybinding { key: "Esc / q", action: "Close Settings", description: "Close settings and return to normal mode" },
                Keybinding { key: "Ctrl+S", action: "Save Settings", description: "Save changes to the current tab's configuration file" },
                Keybinding { key: "Tab", action: "Next Tab", description: "Move to the next settings tab" },
                Keybinding { key: "Shift+Tab", action: "Previous Tab", description: "Move to the previous settings tab" },
                Keybinding { key: "1-4", action: "Jump to Tab", description: "Jump to Project (1), User (2), Launch (3), or VSCode (4)" },
                Keybinding { key: "j / k", action: "Navigate", description: "Move to the next/previous setting" },
                Keybinding { key: "Enter / Space", action: "Edit / Toggle", description: "Edit the selected setting or toggle booleans/enums" },
                Keybinding { key: "+ / -", action: "Inc / Dec", description: "Increment or decrement number values" },
                Keybinding { key: "Esc", action: "Cancel Edit", description: "Cancel editing and discard changes (when editing)" },
            ],
        },

        // ── Confirm Dialog ───────────────────────────────────────────
        KeybindingSection {
            title: "Confirm Dialog",
            color: "bg-red-500",
            key_color: "text-red-400",
            bindings: vec![
                Keybinding { key: "y / Y / Enter", action: "Confirm", description: "Confirm and quit Flutter Demon" },
                Keybinding { key: "q", action: "Confirm", description: "Enables qq quick quit pattern" },
                Keybinding { key: "n / N / Esc", action: "Cancel", description: "Cancel quit and return to normal mode" },
                Keybinding { key: "Ctrl+C", action: "Force Quit", description: "Emergency exit, bypasses confirmation" },
            ],
        },
    ]
}
