use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::code_block::CodeBlock;
use crate::components::icons::{Cpu, Keyboard, Layout, Search, Settings, Smartphone, Terminal, Zap};

#[component]
pub fn Introduction() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-4xl font-bold text-white">"Flutter Demon"</h1>
            <p class="text-lg text-slate-400">
                "Flutter Demon ("<code class="text-blue-400">"fdemon"</code>") is a high-performance terminal user interface for Flutter development, written in Rust. \
                 Run your Flutter apps, view logs in real-time, hot reload on file changes, and manage \
                 multiple device sessions \u{2014} all from the comfort of your terminal."
            </p>

            // ── Why fdemon? ──────────────────────────────────────────
            <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                <FeatureCard
                    icon=|| view! { <Terminal class="w-5 h-5 text-blue-400" /> }.into_any()
                    title="Keyboard-First"
                    text="Vim-style navigation, search, and controls. Never reach for the mouse."
                />
                <FeatureCard
                    icon=|| view! { <Zap class="w-5 h-5 text-yellow-400" /> }.into_any()
                    title="Blazingly Fast"
                    text="Written in Rust. Instant startup, minimal memory, zero lag even with large logs."
                />
                <FeatureCard
                    icon=|| view! { <Smartphone class="w-5 h-5 text-green-400" /> }.into_any()
                    title="Multi-Device"
                    text="Run up to 9 simultaneous sessions. Debug iOS, Android, and Web at the same time."
                />
                <FeatureCard
                    icon=|| view! { <Layout class="w-5 h-5 text-purple-400" /> }.into_any()
                    title="Beautiful TUI"
                    text="Built with Ratatui. Scrollable logs, session tabs, syntax highlighting, and collapsible stack traces."
                />
                <FeatureCard
                    icon=|| view! { <Search class="w-5 h-5 text-cyan-400" /> }.into_any()
                    title="Smart Discovery"
                    text="Auto-detects Flutter Apps, Plugins, and Packages. Discovers example apps in plugins automatically."
                />
                <FeatureCard
                    icon=|| view! { <Settings class="w-5 h-5 text-orange-400" /> }.into_any()
                    title="Fully Configurable"
                    text="TOML-based config with built-in settings panel. Auto-imports VSCode launch.json configurations."
                />
            </div>

            // ── Quick Start ──────────────────────────────────────────
            <Section title="Quick Start">
                <CodeBlock code="# From a Flutter app directory\ncd /path/to/my_flutter_app\nfdemon\n\n# Or with an explicit path\nfdemon /path/to/my_flutter_app" />
                <p class="text-slate-400">
                    "The New Session Dialog appears automatically. Select a device, configure launch settings if needed, and press "
                    <code class="text-blue-400">"Enter"</code>
                    " to launch. Press "<code class="text-blue-400">"d"</code>" anytime to add more device sessions."
                </p>
            </Section>

            // ── Key Features ─────────────────────────────────────────
            <Section title="Key Features">
                <h3 class="text-lg font-bold text-white">"Auto Hot Reload"</h3>
                <p class="text-slate-400">
                    "A built-in file watcher monitors your "<code class="text-blue-400">"lib/"</code>" directory and triggers \
                     hot reload on save with smart debouncing (default 500ms). Watch paths, extensions, and debounce \
                     timing are all configurable."
                </p>

                <h3 class="text-lg font-bold text-white mt-6">"Log Filtering & Search"</h3>
                <p class="text-slate-400">
                    "Filter logs by level (errors, warnings, info, debug) with "
                    <code class="text-blue-400">"f"</code>
                    ", or by source (app, daemon, Flutter, watcher) with "
                    <code class="text-blue-400">"F"</code>
                    ". Use "<code class="text-blue-400">"/"</code>" for regex search (vim-style) and navigate matches with "
                    <code class="text-blue-400">"n"</code>"/"<code class="text-blue-400">"N"</code>
                    ". Jump between errors with "<code class="text-blue-400">"e"</code>"/"<code class="text-blue-400">"E"</code>"."
                </p>

                <h3 class="text-lg font-bold text-white mt-6">"Link Highlight Mode"</h3>
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"L"</code>" to highlight all file references in the viewport with shortcut badges. \
                     Press the corresponding key to open that file in your editor. Auto-detects your IDE when running in an integrated terminal \
                     (VS Code, Cursor, Zed, IntelliJ, Neovim)."
                </p>

                <h3 class="text-lg font-bold text-white mt-6">"Collapsible Stack Traces"</h3>
                <p class="text-slate-400">
                    "Stack traces start collapsed, showing only the first few frames. Press "
                    <code class="text-blue-400">"Enter"</code>
                    " to expand them. The number of visible frames when collapsed is configurable."
                </p>

                <h3 class="text-lg font-bold text-white mt-6">"Built-in Settings Panel"</h3>
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">","</code>" (comma) to open a full-screen settings panel with 4 tabs: \
                     Project Settings, User Preferences, Launch Config, and VSCode Config. Edit everything without touching config files."
                </p>
            </Section>

            // ── Essential Keybindings ────────────────────────────────
            <Section title="Essential Keybindings">
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Key"</th>
                                <th class="p-4 font-medium">"Action"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <KeyRow key="q / Esc" action="Quit (with confirmation if sessions running)" />
                            <KeyRow key="r / R" action="Hot reload / Hot restart" />
                            <KeyRow key="d / +" action="Open New Session Dialog" />
                            <KeyRow key="1-9" action="Switch to session by number" />
                            <KeyRow key="Tab" action="Cycle to next session" />
                            <KeyRow key="j / k" action="Scroll down / up (vim-style)" />
                            <KeyRow key="f / F" action="Cycle level / source filter" />
                            <KeyRow key="/" action="Search logs (regex)" />
                            <KeyRow key="e / E" action="Jump to next / previous error" />
                            <KeyRow key="L" action="Enter link highlight mode" />
                            <KeyRow key="," action="Open settings panel" />
                            <KeyRow key="c" action="Clear logs" />
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 text-sm mt-2">
                    "See the full "<A href="/docs/keybindings" attr:class="text-blue-400 hover:underline">"Keybindings"</A>" reference for all controls across every mode."
                </p>
            </Section>

            // ── Configuration ────────────────────────────────────────
            <Section title="Configuration at a Glance">
                <p class="text-slate-400 mb-4">
                    "All configuration is optional. Flutter Demon works out-of-the-box with sensible defaults."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"File"</th>
                                <th class="p-4 font-medium">"Purpose"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">".fdemon/config.toml"</td>
                                <td class="p-4 text-slate-300">"Project settings \u{2014} behavior, watcher, UI, editor"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">".fdemon/launch.toml"</td>
                                <td class="p-4 text-slate-300">"Launch configs \u{2014} device, mode, flavor, dart-defines"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">".vscode/launch.json"</td>
                                <td class="p-4 text-slate-300">"Auto-imported VSCode configs (read-only)"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 text-sm mt-2">
                    "See the full "<A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>" reference for all options."
                </p>
            </Section>

            // ── Architecture ─────────────────────────────────────────
            <Section title="Architecture">
                <p class="text-slate-400">
                    "Flutter Demon follows "<strong class="text-white">"The Elm Architecture (TEA)"</strong>
                    " pattern for predictable state management."
                </p>
                <div class="bg-slate-900 rounded-lg border border-slate-800 p-6 font-mono text-xs md:text-sm text-slate-300 overflow-x-auto mt-4">
                    <pre class="leading-relaxed">"Events (keyboard, daemon, watcher)\n       \u{2193}\n   Message enum\n       \u{2193}\nupdate(state, message) \u{2192} (new_state, Option<Action>)\n       \u{2193}\nrender(state) \u{2192} Terminal UI\n       \u{2193}\nAction \u{2192} Async tasks (reload, spawn, etc.)"</pre>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-6">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <div class="flex items-center mb-2">
                            <Layout class="w-4 h-4 text-purple-400 mr-2" />
                            <h4 class="font-bold text-white">"TUI Layer"</h4>
                        </div>
                        <p class="text-xs text-slate-400">"Ratatui-based terminal UI with widgets for logs, sessions, dialogs, and settings."</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <div class="flex items-center mb-2">
                            <Cpu class="w-4 h-4 text-blue-400 mr-2" />
                            <h4 class="font-bold text-white">"Daemon Layer"</h4>
                        </div>
                        <p class="text-xs text-slate-400">"Manages Flutter processes via JSON-RPC (--machine mode). Tracks request/response pairs."</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <div class="flex items-center mb-2">
                            <Keyboard class="w-4 h-4 text-green-400 mr-2" />
                            <h4 class="font-bold text-white">"Core Layer"</h4>
                        </div>
                        <p class="text-xs text-slate-400">"Domain types (LogEntry, AppPhase), project discovery, and stack trace parsing. Zero dependencies."</p>
                    </div>
                </div>
                <p class="text-slate-400 text-sm mt-2">
                    "See the "<A href="/docs/architecture" attr:class="text-blue-400 hover:underline">"Architecture"</A>" page for more details."
                </p>
            </Section>

            // ── Tech Stack ───────────────────────────────────────────
            <Section title="Built With">
                <div class="flex flex-wrap gap-2">
                    <TechBadge name="Rust" />
                    <TechBadge name="Ratatui" />
                    <TechBadge name="Crossterm" />
                    <TechBadge name="Tokio" />
                    <TechBadge name="Serde" />
                    <TechBadge name="Notify" />
                    <TechBadge name="Tracing" />
                </div>
            </Section>

            // ── Next Steps ───────────────────────────────────────────
            <Section title="Next Steps">
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <A href="/docs/installation" attr:class="block p-4 bg-slate-900 rounded-lg border border-slate-800 hover:border-slate-700 transition-colors group">
                        <h4 class="font-bold text-white mb-1 group-hover:text-blue-400 transition-colors">"Installation \u{2192}"</h4>
                        <p class="text-sm text-slate-400">"Build from source and get running in minutes."</p>
                    </A>
                    <A href="/docs/keybindings" attr:class="block p-4 bg-slate-900 rounded-lg border border-slate-800 hover:border-slate-700 transition-colors group">
                        <h4 class="font-bold text-white mb-1 group-hover:text-blue-400 transition-colors">"Keybindings \u{2192}"</h4>
                        <p class="text-sm text-slate-400">"Complete reference for all keyboard controls."</p>
                    </A>
                    <A href="/docs/configuration" attr:class="block p-4 bg-slate-900 rounded-lg border border-slate-800 hover:border-slate-700 transition-colors group">
                        <h4 class="font-bold text-white mb-1 group-hover:text-blue-400 transition-colors">"Configuration \u{2192}"</h4>
                        <p class="text-sm text-slate-400">"Customize behavior, watcher, UI, and editor settings."</p>
                    </A>
                    <A href="/docs/architecture" attr:class="block p-4 bg-slate-900 rounded-lg border border-slate-800 hover:border-slate-700 transition-colors group">
                        <h4 class="font-bold text-white mb-1 group-hover:text-blue-400 transition-colors">"Architecture \u{2192}"</h4>
                        <p class="text-sm text-slate-400">"Learn about the TEA pattern and internal design."</p>
                    </A>
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
fn FeatureCard(icon: fn() -> AnyView, title: &'static str, text: &'static str) -> impl IntoView {
    view! {
        <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
            <div class="flex items-center mb-2">
                {icon()}
                <h3 class="font-bold text-white ml-2">{title}</h3>
            </div>
            <p class="text-sm text-slate-400">{text}</p>
        </div>
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
fn TechBadge(name: &'static str) -> impl IntoView {
    view! {
        <span class="px-3 py-1 text-sm font-medium rounded-full bg-slate-800 text-slate-300 border border-slate-700">
            {name}
        </span>
    }
}
