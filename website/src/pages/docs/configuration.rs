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
                <CodeBlock language="toml" code="[behavior]\nauto_start = false      # Skip device selector, use first available device\nconfirm_quit = true     # Show confirmation when quitting with active sessions" />
                <SettingsTable entries=vec![
                    ("auto_start", "boolean", "false", "If true, skips device selector on startup and uses first available device"),
                    ("confirm_quit", "boolean", "true", "If true, shows confirmation dialog when quitting with running apps"),
                ] />
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
                <CodeBlock language="toml" code="[ui]\nlog_buffer_size = 10000         # Max log entries in memory\nshow_timestamps = true          # Display timestamps\ncompact_logs = false            # Collapse similar entries\ntheme = \"default\"               # Color theme\nstack_trace_collapsed = true    # Start stack traces collapsed\nstack_trace_max_frames = 3     # Frames shown when collapsed" />
                <SettingsTable entries=vec![
                    ("log_buffer_size", "integer", "10000", "Max log entries to retain. Older entries are discarded"),
                    ("show_timestamps", "boolean", "true", "Display timestamps for each log entry"),
                    ("compact_logs", "boolean", "false", "Collapse similar consecutive log entries"),
                    ("theme", "string", "\"default\"", "Color theme name"),
                    ("stack_trace_collapsed", "boolean", "true", "Stack traces start collapsed by default"),
                    ("stack_trace_max_frames", "integer", "3", "Frames to show when collapsed. Press Enter to expand"),
                ] />
            </Section>

            // ── DevTools Settings ────────────────────────────────────
            <Section title="DevTools Settings">
                <CodeBlock language="toml" code="[devtools]\nauto_open = false          # Auto-open DevTools on app start\nbrowser = \"\"               # Browser command (empty = system default)" />
                <SettingsTable entries=vec![
                    ("auto_open", "boolean", "false", "Automatically open DevTools in a browser when app starts"),
                    ("browser", "string", "\"\"", "Browser command (e.g. \"chrome\", \"firefox\"). Empty = system default"),
                ] />
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

            // ── Complete Example ──────────────────────────────────────
            <Section title="Complete Example">
                <h3 class="text-lg font-bold text-white">"config.toml"</h3>
                <CodeBlock language="toml" code="[behavior]\nauto_start = false\nconfirm_quit = true\n\n[watcher]\npaths = [\"lib\", \"packages/core/lib\"]\ndebounce_ms = 500\nauto_reload = true\nextensions = [\"dart\"]\n\n[ui]\nlog_buffer_size = 15000\nshow_timestamps = true\ncompact_logs = false\nstack_trace_collapsed = true\nstack_trace_max_frames = 3\n\n[devtools]\nauto_open = false\n\n[editor]\ncommand = \"\"  # Auto-detect" />

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
fn SettingsTable(entries: Vec<(&'static str, &'static str, &'static str, &'static str)>) -> impl IntoView {
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
