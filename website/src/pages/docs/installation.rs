use leptos::prelude::*;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Installation() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Installation"</h1>
            <p class="text-lg text-slate-400">
                "Flutter Demon can be installed via the install script (recommended), downloaded as a \
                 pre-built binary, or built from source."
            </p>

            // ── Quick Install ─────────────────────────────────────────
            <Section title="Quick Install">
                <div class="bg-green-900/20 border border-green-800 p-4 rounded-lg text-green-200 text-sm">
                    <p class="font-medium mb-1">"Recommended: one-liner for macOS and Linux"</p>
                    <p>"This downloads the latest release binary for your platform and installs it to "
                    <code class="text-green-300">"$HOME/.local/bin"</code>"."</p>
                </div>
                <CodeBlock code="curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/master/install.sh | bash" />
            </Section>

            // ── Specifying a Version ──────────────────────────────────
            <Section title="Specifying a Version">
                <p class="text-slate-400">
                    "To install a specific release, pass the "<code class="text-blue-400">"--version"</code>
                    " flag to the script:"
                </p>
                <CodeBlock code="curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/master/install.sh | bash -s -- --version 0.1.0" />
            </Section>

            // ── Custom Install Directory ──────────────────────────────
            <Section title="Custom Install Directory">
                <p class="text-slate-400">
                    "Set the "<code class="text-blue-400">"FDEMON_INSTALL_DIR"</code>
                    " environment variable to install to a different directory:"
                </p>
                <CodeBlock code="FDEMON_INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/master/install.sh | bash" />
            </Section>

            // ── Supported Platforms ───────────────────────────────────
            <Section title="Supported Platforms">
                <p class="text-slate-400">
                    "Pre-built binaries are available for the following platforms and architectures:"
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Platform"</th>
                                <th class="p-4 font-medium">"Architecture"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"Target"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white">"macOS"</td>
                                <td class="p-4 text-slate-300">"Intel (x86_64)"</td>
                                <td class="p-4 font-mono text-blue-400 text-xs hidden md:table-cell">"x86_64-apple-darwin"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white">"macOS"</td>
                                <td class="p-4 text-slate-300">"Apple Silicon (aarch64)"</td>
                                <td class="p-4 font-mono text-blue-400 text-xs hidden md:table-cell">"aarch64-apple-darwin"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white">"Linux"</td>
                                <td class="p-4 text-slate-300">"x86_64"</td>
                                <td class="p-4 font-mono text-blue-400 text-xs hidden md:table-cell">"x86_64-unknown-linux-gnu"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white">"Linux"</td>
                                <td class="p-4 text-slate-300">"ARM64 (aarch64)"</td>
                                <td class="p-4 font-mono text-blue-400 text-xs hidden md:table-cell">"aarch64-unknown-linux-gnu"</td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 text-white">"Windows"</td>
                                <td class="p-4 text-slate-300">"x86_64"</td>
                                <td class="p-4 font-mono text-blue-400 text-xs hidden md:table-cell">"x86_64-pc-windows-msvc"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Manual Download (Windows) ─────────────────────────────
            <Section title="Manual Download (Windows)">
                <p class="text-slate-400">
                    "For Windows, download the "<code class="text-blue-400">".zip"</code>
                    " archive from the "
                    <a
                        href="https://github.com/edTheGuy00/fdemon/releases"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="text-blue-400 hover:underline"
                    >
                        "GitHub Releases"
                    </a>
                    " page. Extract the archive and add "<code class="text-blue-400">"fdemon.exe"</code>
                    " to a directory on your "<code class="text-blue-400">"PATH"</code>"."
                </p>
                <ol class="list-decimal list-inside text-slate-400 space-y-2 ml-2 mt-3">
                    <li>"Download the "<code class="text-blue-400">"x86_64-pc-windows-msvc.zip"</code>" asset from the latest release."</li>
                    <li>"Extract the archive to a folder, for example "<code class="text-blue-400">"C:\\tools\\fdemon\\"</code>"."</li>
                    <li>"Add that folder to your "<code class="text-blue-400">"PATH"</code>" via System Properties \u{2192} Environment Variables."</li>
                    <li>"Open a new terminal and run "<code class="text-blue-400">"fdemon --version"</code>" to verify."</li>
                </ol>
            </Section>

            // ── Build from Source ─────────────────────────────────────
            <Section title="Build from Source">
                <p class="text-slate-400">
                    "Build Flutter Demon from source if you need a custom build or your platform is not \
                     listed above."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-2">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Requirement"</th>
                                <th class="p-4 font-medium">"Details"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"Rust 1.70+"</td>
                                <td class="p-4 text-slate-300">"Install via "<a href="https://rustup.rs" target="_blank" rel="noopener noreferrer" class="text-blue-400 hover:underline">"rustup.rs"</a></td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400">"Flutter SDK"</td>
                                <td class="p-4 text-slate-300">"Required for running Flutter projects, not for building fdemon"</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <CodeBlock code="git clone https://github.com/edTheGuy00/fdemon.git\ncd flutter-demon\ncargo build --release\n# Binary is at ./target/release/fdemon" />
            </Section>

            // ── Verifying Installation ────────────────────────────────
            <Section title="Verifying Installation">
                <p class="text-slate-400">
                    "After installation, run the following command to confirm "<code class="text-blue-400">"fdemon"</code>
                    " is on your PATH and working:"
                </p>
                <CodeBlock code="fdemon --version" />
                <p class="text-slate-400 text-sm">
                    "Expected output: "<code class="text-blue-400">"fdemon 0.1.0"</code>" (or the installed version)."
                </p>
            </Section>

            // ── PATH Setup ────────────────────────────────────────────
            <Section title="PATH Setup">
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm">
                    <p class="font-medium mb-2">"fdemon not found after installation?"</p>
                    <p>
                        "Ensure "<code class="text-blue-300">"$HOME/.local/bin"</code>
                        " is in your PATH. Add the following line to your shell profile \
                         ("<code class="text-blue-300">"~/.bashrc"</code>", "
                        <code class="text-blue-300">"~/.zshrc"</code>", etc.) and restart your terminal:"
                    </p>
                    <pre class="mt-2 font-mono text-blue-300 bg-blue-950/40 px-3 py-2 rounded text-xs overflow-x-auto">
                        {"export PATH=\"$HOME/.local/bin:$PATH\""}
                    </pre>
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
