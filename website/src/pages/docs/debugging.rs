use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Debugging() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"DAP Debugging"</h1>
            <p class="text-lg text-slate-400">
                "Connect your IDE\u{2019}s debugger to a running fdemon session \u{2014} set breakpoints, \
                 step through code, inspect variables, all while fdemon manages the Flutter process."
            </p>

            // ── Overview ─────────────────────────────────────────────
            <Section title="Overview">
                <p class="text-slate-400">
                    "Flutter Demon implements the Debug Adapter Protocol (DAP), a language-agnostic wire format \
                     that IDEs use to communicate with debuggers. Instead of each IDE needing a Flutter-specific \
                     plugin, any editor that speaks DAP can attach to a running fdemon session."
                </p>
                <p class="text-slate-400 mt-3">
                    "fdemon uses an attach model: fdemon owns the Flutter process and its VM Service connection. \
                     The IDE attaches to fdemon via DAP \u{2014} it does not launch Flutter itself. This avoids \
                     the dual VM Service connection conflicts that occur when an IDE and a process manager both \
                     try to own the same Dart process."
                </p>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Breakpoints"</h4>
                        <p class="text-sm text-slate-400">"Set, clear, and hit breakpoints in Dart source. Supports conditional breakpoints and logpoints."</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Variable Inspection"</h4>
                        <p class="text-sm text-slate-400">"Browse the call stack, inspect local variables, and evaluate expressions in the REPL when paused."</p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Hot Reload Integration"</h4>
                        <p class="text-sm text-slate-400">"Trigger hot reload and hot restart directly from your IDE via custom DAP requests while fdemon stays in control."</p>
                    </div>
                </div>
            </Section>

            // ── Quick Start ───────────────────────────────────────────
            <Section title="Quick Start">
                <p class="text-slate-400">
                    "Three steps to connect your IDE to a running fdemon session:"
                </p>

                <div class="space-y-4 mt-4">
                    <div class="flex gap-4">
                        <div class="flex-shrink-0 w-8 h-8 rounded-full bg-blue-500 flex items-center justify-center text-white font-bold text-sm">"1"</div>
                        <div class="flex-1">
                            <p class="text-white font-medium mb-1">"Run fdemon in your Flutter project"</p>
                            <CodeBlock language="bash" code="fdemon" />
                        </div>
                    </div>

                    <div class="flex gap-4">
                        <div class="flex-shrink-0 w-8 h-8 rounded-full bg-blue-500 flex items-center justify-center text-white font-bold text-sm">"2"</div>
                        <div class="flex-1">
                            <p class="text-white font-medium mb-1">"Press " <code class="text-blue-400">"D"</code> " to start the DAP server"</p>
                            <p class="text-slate-400 text-sm">"The status bar shows the assigned port, e.g. "<code class="text-blue-400">"DAP :4711"</code>". \
                               If running in a detected IDE terminal, the DAP server starts automatically."</p>
                        </div>
                    </div>

                    <div class="flex gap-4">
                        <div class="flex-shrink-0 w-8 h-8 rounded-full bg-blue-500 flex items-center justify-center text-white font-bold text-sm">"3"</div>
                        <div class="flex-1">
                            <p class="text-white font-medium mb-1">"Connect your IDE to the DAP server"</p>
                            <CodeBlock language="bash" code="# TCP attach — replace 4711 with the port shown in the status bar
127.0.0.1:4711" />
                        </div>
                    </div>
                </div>

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Tip:"</strong>
                    " When "<code class="text-blue-300">"dap.auto_configure_ide = true"</code>" (the default), fdemon automatically \
                     writes the correct launch config for your IDE when the server starts. Check \
                     "<code class="text-blue-300">".fdemon/"</code>" for the generated file."
                </div>
            </Section>

            // ── Transport Modes ───────────────────────────────────────
            <Section title="Transport Modes">
                <p class="text-slate-400">
                    "fdemon supports two DAP transport modes. TCP is recommended for interactive use."
                </p>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-2">"TCP (Recommended)"</h4>
                        <p class="text-sm text-slate-400 mb-3">
                            "fdemon binds a TCP socket and waits for an IDE to connect. The TUI remains fully \
                             interactive while the debugger is attached."
                        </p>
                        <CodeBlock language="bash" code="# Start fdemon, then press D in the TUI
# Or set dap.enabled = true in .fdemon/config.toml to always start
fdemon" />
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-2">"Stdio (Testing Only)"</h4>
                        <p class="text-sm text-slate-400 mb-3">
                            "The IDE launches fdemon as a subprocess and communicates over stdin/stdout. \
                             The TUI is not rendered in this mode."
                        </p>
                        <CodeBlock language="bash" code="# IDE subprocess launch — not for interactive use
fdemon --dap-stdio" />
                    </div>
                </div>

                <div class="bg-yellow-900/20 border border-yellow-800 p-4 rounded-lg text-yellow-200 text-sm mt-4">
                    <strong>"Note:"</strong>
                    " Stdio mode disables the TUI entirely. Hot reload, session management, and log viewing \
                     are unavailable. Use TCP mode for any workflow where you want to keep the fdemon TUI \
                     running alongside your IDE."
                </div>
            </Section>

            // ── Automatic IDE Configuration ───────────────────────────
            <Section title="Automatic IDE Configuration">
                <p class="text-slate-400">
                    "When the DAP server starts, fdemon detects which IDE you are running in and writes a \
                     ready-to-use debug configuration to disk. The next time you open your project, the \
                     debugger entry will already be present."
                </p>

                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"IDE"</th>
                                <th class="p-4 font-medium">"Env Var Checked"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Config File Written"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white font-medium">"VS Code"</td>
                                <td class="p-4 font-mono text-slate-400 text-xs">"TERM_PROGRAM=vscode"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">".vscode/launch.json"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white font-medium">"Zed"</td>
                                <td class="p-4 font-mono text-slate-400 text-xs">"ZED_TERM=1"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">".zed/debug.json"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white font-medium">"Neovim"</td>
                                <td class="p-4 font-mono text-slate-400 text-xs">"NVIM / NVIM_LISTEN_ADDRESS"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">".fdemon/dap-nvim.lua"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white font-medium">"Helix"</td>
                                <td class="p-4 font-mono text-slate-400 text-xs">"HELIX_RUNTIME / helix process"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">".fdemon/dap-helix.txt"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white font-medium">"Emacs"</td>
                                <td class="p-4 font-mono text-slate-400 text-xs">"INSIDE_EMACS / EMACS"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">".fdemon/dap-emacs.el"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <h3 class="text-lg font-bold text-white mt-6">"CLI: Generate Config Manually"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "You can trigger config generation without starting a full fdemon session using the \
                     "<code class="text-blue-400">"--dap-config"</code>" flag:"
                </p>
                <CodeBlock language="bash" code="# Write VS Code launch.json for port 4711
fdemon --dap-config vscode --dap-port 4711

# Write Neovim config
fdemon --dap-config neovim --dap-port 4711" />

                <h3 class="text-lg font-bold text-white mt-6">"Disabling Auto-Configuration"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "To stop fdemon from writing IDE config files, set the following in \
                     "<code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>":"
                </p>
                <CodeBlock language="toml" code="[dap]
auto_configure_ide = false" />
            </Section>

            // ── IDE Setup ─────────────────────────────────────────────
            <Section title="IDE Setup">
                <p class="text-slate-400">
                    "Manual configuration snippets for each supported IDE. Replace "<code class="text-blue-400">"4711"</code>
                    " with the port shown in the fdemon status bar."
                </p>

                // Zed
                <h3 class="text-lg font-bold text-white mt-6">"Zed"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "Add to "<code class="text-blue-400 bg-slate-900 px-1 rounded">".zed/debug.json"</code>" in your project:"
                </p>
                <CodeBlock language="json" code=r#"[
  {
    "label": "Flutter Demon (TCP)",
    "adapter": "Delve",
    "request": "attach",
    "tcp_connection": {
      "host": "127.0.0.1",
      "port": 4711
    }
  }
]"# />

                // Helix
                <h3 class="text-lg font-bold text-white mt-6">"Helix"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "Connect from the Helix command prompt after fdemon\u{2019}s DAP server is running:"
                </p>
                <CodeBlock language="text" code=":debug-remote 127.0.0.1:4711" />

                // Neovim
                <h3 class="text-lg font-bold text-white mt-6">"Neovim (nvim-dap)"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "Add to your nvim-dap configuration (e.g. "<code class="text-blue-400 bg-slate-900 px-1 rounded">"~/.config/nvim/lua/dap.lua"</code>"):"
                </p>
                <CodeBlock language="lua" code=r#"local dap = require('dap')

dap.adapters.fdemon_tcp = {
  type = 'server',
  host = '127.0.0.1',
  port = 4711,
}

dap.configurations.dart = {
  {
    type = 'fdemon_tcp',
    request = 'attach',
    name = 'Flutter Demon (TCP)',
  },
}"# />

                // VS Code
                <h3 class="text-lg font-bold text-white mt-6">"VS Code"</h3>
                <p class="text-slate-400 text-sm mb-2">
                    "Add to "<code class="text-blue-400 bg-slate-900 px-1 rounded">".vscode/launch.json"</code>":"
                </p>
                <CodeBlock language="json" code=r#"{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Flutter Demon (TCP)",
      "type": "node",
      "request": "attach",
      "debugServer": 4711
    }
  ]
}"# />

                // Emacs
                <h3 class="text-lg font-bold text-white mt-6">"Emacs (dap-mode)"</h3>
                <p class="text-slate-400 text-sm">
                    "When auto-configuration is enabled, fdemon writes \
                     "<code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/dap-emacs.el"</code>
                    " to your project. Load it from your init file or evaluate it with \
                     "<code class="text-blue-400">"M-x load-file"</code>". The generated file registers \
                     a dap-mode debug template pointing to the active port."
                </p>
            </Section>

            // ── Debugging Features ────────────────────────────────────
            <Section title="Debugging Features">
                <p class="text-slate-400">
                    "fdemon\u{2019}s DAP implementation covers the full interactive debugging workflow:"
                </p>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Conditional Breakpoints"</h4>
                        <p class="text-sm text-slate-400">
                            "Set a "<code class="text-xs text-blue-400">"condition"</code>" expression that must be true \
                             before the breakpoint fires, or a "<code class="text-xs text-blue-400">"hitCondition"</code>
                             " to trigger only on the N-th hit."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Logpoints"</h4>
                        <p class="text-sm text-slate-400">
                            "Provide a "<code class="text-xs text-blue-400">"logMessage"</code>" with \
                             "<code class="text-xs text-blue-400">"{expression}"</code>" interpolation. \
                             The message is logged to fdemon\u{2019}s output without pausing execution."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Expression Evaluation"</h4>
                        <p class="text-sm text-slate-400">
                            "Evaluate Dart expressions in hover, watch, repl, and clipboard contexts \
                             while the isolate is paused at a breakpoint."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Source References"</h4>
                        <p class="text-sm text-slate-400">
                            "Step into SDK and package source files via \
                             "<code class="text-xs text-blue-400">"sourceReference"</code>
                             ". Content is fetched from the VM Service on demand."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Hot Reload via DAP"</h4>
                        <p class="text-sm text-slate-400">
                            "Trigger hot reload or hot restart from your IDE using the custom \
                             "<code class="text-xs text-blue-400">"hotReload"</code>" and \
                             "<code class="text-xs text-blue-400">"hotRestart"</code>" DAP requests \
                             without leaving the debugger."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Auto-Reload Suppression"</h4>
                        <p class="text-sm text-slate-400">
                            "When the debugger is paused at a breakpoint, fdemon\u{2019}s file watcher \
                             automatically suspends hot reload triggers to prevent interference. \
                             Reload resumes when you continue execution."
                        </p>
                    </div>
                </div>
            </Section>

            // ── Multi-Session Debugging ───────────────────────────────
            <Section title="Multi-Session Debugging">
                <p class="text-slate-400">
                    "fdemon can manage up to 9 concurrent Flutter sessions. Each session\u{2019}s Dart isolates \
                     are exposed to the IDE as threads using a namespaced thread ID scheme. The IDE sees a \
                     single flat thread list; fdemon maps each thread ID back to the correct session and isolate."
                </p>

                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Session"</th>
                                <th class="p-4 font-medium">"Thread ID Range"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Example Thread IDs"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-slate-300">"Session 0"</td>
                                <td class="p-4 font-mono text-blue-400">"1000 \u{2013} 1999"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">"1001 (main isolate), 1002 (background)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-slate-300">"Session 1"</td>
                                <td class="p-4 font-mono text-blue-400">"2000 \u{2013} 2999"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">"2001 (main isolate)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-slate-300">"Session 2"</td>
                                <td class="p-4 font-mono text-blue-400">"3000 \u{2013} 3999"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell font-mono text-xs">"3001 (main isolate)"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-slate-300">"Sessions 3\u{2013}8"</td>
                                <td class="p-4 font-mono text-blue-400">"4000 \u{2013} 9999"</td>
                                <td class="p-4 text-slate-500 hidden md:table-cell">"Same pattern, offset by session index"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <p class="text-slate-400 text-sm mt-3">
                    "Most IDEs show all threads in a single list. Use the thread name (which includes the \
                     session label) to identify which Flutter app a thread belongs to."
                </p>
            </Section>

            // ── DAP Settings ─────────────────────────────────────────
            <Section title="DAP Settings">
                <p class="text-slate-400">
                    "All DAP options live under the "<code class="text-blue-400">"[dap]"</code>" section of \
                     "<code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>
                    ". All settings are optional."
                </p>
                <CodeBlock language="toml" code="[dap]
enabled = false
auto_start_in_ide = true
port = 0
bind_address = \"127.0.0.1\"
suppress_reload_on_pause = true
auto_configure_ide = true" />

                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Setting"</th>
                                <th class="p-4 font-medium">"Default"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Description"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <SettingsRow
                                prop="dap.enabled"
                                default="false"
                                desc="Always start the DAP server at launch, regardless of IDE detection." />
                            <SettingsRow
                                prop="dap.auto_start_in_ide"
                                default="true"
                                desc="Auto-start the DAP server when fdemon is running inside a detected IDE terminal (VS Code, Neovim, Helix, Zed, Emacs). No effect when enabled = true." />
                            <SettingsRow
                                prop="dap.port"
                                default="0 (auto)"
                                desc="TCP port for the DAP server. 0 lets the OS assign an available port. Set a fixed port for stable IDE configs across restarts." />
                            <SettingsRow
                                prop="dap.bind_address"
                                default="\"127.0.0.1\""
                                desc="Network interface to bind. Keep as 127.0.0.1 for local development; change only if you need remote access." />
                            <SettingsRow
                                prop="dap.suppress_reload_on_pause"
                                default="true"
                                desc="Pause the file watcher's hot-reload trigger while the debugger is stopped at a breakpoint. Prevents reload from disrupting the paused state." />
                            <SettingsRow
                                prop="dap.auto_configure_ide"
                                default="true"
                                desc="Automatically generate the appropriate IDE debug config file when the DAP server starts." />
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Troubleshooting ───────────────────────────────────────
            <Section title="Troubleshooting">
                <div class="space-y-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Port already in use"</h4>
                        <p class="text-sm text-slate-400">
                            "Another process is holding the configured port. Use \
                             "<code class="text-xs text-blue-400">"dap.port = 0"</code>
                             " to let the OS assign an available port automatically, or pick a different \
                             fixed port. The assigned port is always shown in the status bar."
                        </p>
                    </div>

                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"fdemon: command not found"</h4>
                        <p class="text-sm text-slate-400">
                            "The fdemon binary is not on your PATH. Add \
                             "<code class="text-xs text-blue-400">"~/.cargo/bin"</code>
                             " to your shell\u{2019}s PATH, or install with \
                             "<code class="text-xs text-blue-400">"cargo install fdemon"</code>"."
                        </p>
                    </div>

                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Breakpoints not hitting after hot restart"</h4>
                        <p class="text-sm text-slate-400">
                            "A hot restart replaces the running Dart program. The VM allocates new isolates \
                             and the debugger loses its breakpoint registrations. Re-set your breakpoints \
                             in the IDE after a hot restart."
                        </p>
                    </div>

                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Auto-reload not triggering on file save"</h4>
                        <p class="text-sm text-slate-400">
                            "The debugger may be paused at a breakpoint. \
                             "<code class="text-xs text-blue-400">"suppress_reload_on_pause"</code>
                             " is enabled by default to prevent reloads from interrupting a debug session. \
                             Resume (continue) execution to re-enable the file watcher."
                        </p>
                    </div>

                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"IDE shows \u{201c}paused\u{201d} but fdemon TUI shows \u{201c}running\u{201d}"</h4>
                        <p class="text-sm text-slate-400">
                            "This can happen when a "<code class="text-xs text-blue-400">"PauseStart"</code>
                            " event arrives before the IDE has fully attached. Click Continue in your IDE \
                             to resume the isolate and re-synchronize state."
                        </p>
                    </div>
                </div>
            </Section>

            // ── Capabilities ─────────────────────────────────────────
            <Section title="Implemented DAP Capabilities">
                <p class="text-slate-400">
                    "The following DAP capabilities are implemented and reported in the \
                     "<code class="text-blue-400">"initialize"</code>" response:"
                </p>

                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Capability"</th>
                                <th class="p-4 font-medium">"Supported"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Notes"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <CapRow cap="supportsConfigurationDoneRequest" supported=true notes="Required handshake after initialize." />
                            <CapRow cap="supportsSetVariable" supported=true notes="Modify local variables while paused." />
                            <CapRow cap="supportsConditionalBreakpoints" supported=true notes="condition expression on SetBreakpoints." />
                            <CapRow cap="supportsHitConditionalBreakpoints" supported=true notes="hitCondition on SetBreakpoints." />
                            <CapRow cap="supportsLogPoints" supported=true notes="logMessage with {expr} interpolation." />
                            <CapRow cap="supportsEvaluateForHovers" supported=true notes="Expression evaluation in hover context." />
                            <CapRow cap="supportsRestartRequest" supported=true notes="Maps to hot restart." />
                            <CapRow cap="supportsRestartFrame" supported=false notes="Frame restart not yet implemented." />
                            <CapRow cap="supportsStepBack" supported=false notes="Dart VM does not support reverse execution." />
                            <CapRow cap="supportsGotoTargetsRequest" supported=false notes="Not implemented." />
                            <CapRow cap="supportsCompletionsRequest" supported=false notes="REPL completion not yet implemented." />
                            <CapRow cap="supportsExceptionOptions" supported=true notes="Break on caught / uncaught exceptions." />
                            <CapRow cap="supportsExceptionInfoRequest" supported=true notes="Exception detail in the IDE." />
                            <CapRow cap="supportsLoadedSourcesRequest" supported=true notes="List all loaded Dart source files." />
                            <CapRow cap="supportsTerminateRequest" supported=true notes="Terminates the Flutter process." />
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Cross-link ────────────────────────────────────────────
            <Section title="Further Reading">
                <p class="text-slate-400">
                    "For detailed per-IDE setup instructions including platform-specific notes, see the \
                     IDE Setup Guide in the repository documentation. For all fdemon keybindings, see the "
                    <A href="/docs/keybindings" attr:class="text-blue-400 hover:underline">"Keybindings"</A>
                    " page. For all configuration options, see the "
                    <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>
                    " page."
                </p>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Repository docs:"</strong>
                    " The "<code class="text-blue-300">"docs/IDE_SETUP.md"</code>
                    " file in the fdemon repository contains the full per-IDE DAP setup reference, \
                     including platform-specific paths and editor plugin version requirements."
                </div>
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

#[component]
fn CapRow(cap: &'static str, supported: bool, notes: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50">
            <td class="p-4 font-mono text-blue-400 text-xs whitespace-nowrap">{cap}</td>
            <td class="p-4 text-center">
                {if supported {
                    view! { <span class="text-green-400 font-bold">"Yes"</span> }.into_any()
                } else {
                    view! { <span class="text-slate-500">"No"</span> }.into_any()
                }}
            </td>
            <td class="p-4 text-slate-500 hidden md:table-cell text-xs">{notes}</td>
        </tr>
    }
}
