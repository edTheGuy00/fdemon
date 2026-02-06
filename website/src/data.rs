use leptos::prelude::*;

use crate::components::icons::{Cpu, Layout, Smartphone, Zap};

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
                Keybinding { key: "+ / d", action: "Start New Session", description: "Open New Session Dialog to configure and launch a session" },
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
