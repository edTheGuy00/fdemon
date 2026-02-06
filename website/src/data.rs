use leptos::prelude::*;

use crate::components::icons::{Layout, Search, Smartphone, Zap};

pub struct Feature {
    pub icon: fn() -> AnyView,
    pub title: &'static str,
    pub desc: &'static str,
}

pub fn features() -> Vec<Feature> {
    vec![
        Feature {
            icon: || view! { <Search class="w-6 h-6 text-blue-400" /> }.into_any(),
            title: "Smart Discovery",
            desc: "Intelligently detects Flutter Apps, Plugins, and Packages. Automatically finds example apps within plugins.",
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

pub fn normal_keybindings() -> Vec<Keybinding> {
    vec![
        Keybinding { key: "q / Esc", action: "Quit", description: "Request to quit (may show confirmation)" },
        Keybinding { key: "r", action: "Hot Reload", description: "Trigger a hot reload (disabled when busy)" },
        Keybinding { key: "R", action: "Hot Restart", description: "Trigger a hot restart (disabled when busy)" },
        Keybinding { key: "d", action: "Open device selector", description: "Open the New Session Dialog" },
        Keybinding { key: "c", action: "Clear logs", description: "Clear all logs in current session" },
        Keybinding { key: "1-9", action: "Switch Session", description: "Switch to session 1-9 directly" },
        Keybinding { key: "Tab", action: "Next Session", description: "Cycle to the next session" },
        Keybinding { key: "j / k", action: "Scroll", description: "Scroll down/up (vim-style)" },
        Keybinding { key: "/", action: "Search", description: "Enter search input mode" },
        Keybinding { key: "L", action: "Link Mode", description: "Enter link highlight mode to open files" },
    ]
}

pub fn dialog_keybindings() -> Vec<Keybinding> {
    vec![
        Keybinding { key: "Tab", action: "Switch Pane", description: "Switch between Target Selector and Launch Context" },
        Keybinding { key: "Enter", action: "Select / Launch", description: "Select device or Launch configuration" },
        Keybinding { key: "\u{2191} / \u{2193}", action: "Navigate", description: "Move selection up or down" },
        Keybinding { key: "r", action: "Refresh", description: "Refresh device list" },
    ]
}

pub fn settings_keybindings() -> Vec<Keybinding> {
    vec![
        Keybinding { key: ",", action: "Open Settings", description: "Open the full-screen settings panel" },
        Keybinding { key: "Enter", action: "Edit Value", description: "Edit the selected setting" },
        Keybinding { key: "Space", action: "Toggle Bool", description: "Toggle boolean values" },
    ]
}
