use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::A;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Toolchain() -> impl IntoView {
    view! {
        <Title text="Install Wizard" />
        <Meta name="description" content="Flutter Demon's guided Install Wizard diagnoses a missing or incomplete Flutter toolchain and installs it for you \u{2014} a managed Flutter SDK, the Android command-line tools, JDK detection, and shell PATH configuration, across Linux, macOS, and Windows." />
        <Link rel="canonical" href="https://fdemon.dev/docs/toolchain" />
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Install Wizard"</h1>
            <p class="text-lg text-slate-400">
                "On a fresh machine, \u{201c}Flutter SDK not found\u{201d} is a dead end. The Install Wizard \
                 replaces that one-line error with a guided, "<code class="text-blue-400">"flutter doctor"</code>
                "-style screen that diagnoses exactly what is missing and walks you through installing it \
                 \u{2014} a managed Flutter SDK, the Android command-line tools, a JDK, and your shell PATH."
            </p>

            // ── Overview ──────────────────────────────────────────────
            <Section title="Overview">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"I"</code>" from the log view to open the wizard. It runs a \
                     toolchain preflight \u{2014} fdemon\u{2019}s own structured checks plus, when Flutter is present, \
                     the embedded "<code class="text-blue-400">"flutter doctor -v"</code>" output \u{2014} and presents \
                     five ordered steps with a roll-up status on each."
                </p>
                <div class="bg-amber-900/20 border border-amber-800 p-4 rounded-lg text-amber-100 text-sm mt-2">
                    <strong>"Hybrid by design:"</strong>
                    " safe, self-contained steps (download/extract the Flutter SDK, fetch the Android \
                     cmdline-tools, run "<code>"sdkmanager"</code>", write your shell rc files) are run \
                     automatically. Steps that need "<code>"sudo"</code>" or a GUI (apt/dnf, "
                    <code>"xcode-select --install"</code>", JDK install, Rosetta) are "
                    <strong>"never auto-run"</strong>" \u{2014} the wizard shows the exact copy-paste command and \
                     re-checks when you\u{2019}re done. fdemon never runs "<code>"sudo"</code>" for you."
                </div>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Diagnose"</h4>
                        <p class="text-sm text-slate-400">
                            "A structured preflight reports Flutter, git, JDK, Android cmdline-tools, \
                             platform-tools, platforms/build-tools, licenses, and OS prerequisites."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Install"</h4>
                        <p class="text-sm text-slate-400">
                            "Installs a managed Flutter SDK (git clone, archive fallback) and the Android \
                             toolchain, with live download progress and a streamed log tail."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Configure"</h4>
                        <p class="text-sm text-slate-400">
                            "Writes idempotent, marker-fenced PATH and "<code class="text-green-400">"ANDROID_HOME"</code>
                            " exports to the right shell rc files, then re-checks until everything is green."
                        </p>
                    </div>
                </div>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Cross-platform:"</strong>
                    " detection and the doctor view work everywhere. Install automation targets "
                    <strong>"Linux, macOS, and Windows"</strong>", following per-OS rules (privileged \
                     prerequisites are guided rather than auto-run)."
                </div>
            </Section>

            // ── The Five Steps ────────────────────────────────────────
            <Section title="The Five Steps">
                <p class="text-slate-400">
                    "The wizard\u{2019}s left pane lists five ordered steps; the right pane shows per-step \
                     detail \u{2014} the underlying component checks, guided commands, and live install progress. \
                     Each step shows a roll-up status (OK / Partial / Missing / Pending)."
                </p>

                <div class="bg-slate-900 rounded-lg border border-slate-800 p-4 font-mono text-xs text-slate-400 overflow-x-auto mt-4">
                    <pre class="leading-relaxed">{"\
\u{250c}\u{2500} Install Wizard \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}
\u{2502} Steps                 \u{2502} Flutter SDK                            \u{2502}
\u{2502} \u{2714} Prerequisites      \u{2502}   [\u{2714}] flutter 3.x \u{00b7} stable             \u{2502}
\u{2502} \u{2716} Android Tools      \u{2502}   ~/fvm/versions/stable/bin/flutter    \u{2502}
\u{2502} \u{2714} Flutter SDK        \u{2502}                                        \u{2502}
\u{2502} \u{25cb} PATH Config        \u{2502}   Enter: run \u{00b7} r: re-check             \u{2502}
\u{2502} \u{25cb} Doctor             \u{2502}                                        \u{2502}
\u{2514}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2534}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2518}"}</pre>
                </div>

                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Step"</th>
                                <th class="p-4 font-medium">"Mode"</th>
                                <th class="p-4 font-medium hidden md:table-cell">"What it does"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"1. Prerequisites"</td>
                                <td class="p-4 text-amber-300 whitespace-nowrap">"Guided"</td>
                                <td class="p-4 text-slate-300 hidden md:table-cell">
                                    "OS-level packages \u{2014} cmake, ninja, clang on Linux; Xcode CLT, CocoaPods, \
                                     Rosetta on macOS; git on Windows. Shown as copy-paste commands."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"2. Android Tools"</td>
                                <td class="p-4 text-green-300 whitespace-nowrap">"Auto (JDK-gated)"</td>
                                <td class="p-4 text-slate-300 hidden md:table-cell">
                                    "Downloads cmdline-tools, relocates to "<code>"cmdline-tools/latest/"</code>", \
                                     accepts licenses, and runs "<code>"sdkmanager"</code>" for platform-tools, \
                                     platform, and build-tools. Blocks until a JDK 17 is present (guided)."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"3. Flutter SDK"</td>
                                <td class="p-4 text-green-300 whitespace-nowrap">"Auto"</td>
                                <td class="p-4 text-slate-300 hidden md:table-cell">
                                    "Installs a managed Flutter SDK via "<code>"git clone"</code>" (default) with an \
                                     archive-download fallback, then runs "<code>"flutter precache"</code>"."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"4. PATH Config"</td>
                                <td class="p-4 text-green-300 whitespace-nowrap">"Auto"</td>
                                <td class="p-4 text-slate-300 hidden md:table-cell">
                                    "Writes marker-fenced PATH and "<code>"ANDROID_HOME"</code>" exports to your \
                                     shell rc files (idempotent). Runs after a successful install."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"5. Doctor"</td>
                                <td class="p-4 text-slate-400 whitespace-nowrap">"Display-only"</td>
                                <td class="p-4 text-slate-300 hidden md:table-cell">
                                    "Embeds the real "<code>"flutter doctor -v"</code>" output once Flutter exists, \
                                     parsing the "<code>"[\u{2714}] [!] [\u{2716}]"</code>" status prefixes for display."
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Step order vs. install order:"</strong>
                    " the UI order mirrors the dependency order (prerequisites \u{2192} Android tools \u{2192} \
                     Flutter SDK \u{2192} PATH \u{2192} doctor). The wizard skips any step the preflight already \
                     found satisfied."
                </div>
            </Section>

            // ── Preflight Checks ──────────────────────────────────────
            <Section title="Preflight Checks">
                <p class="text-slate-400">
                    "Because "<code class="text-blue-400">"flutter doctor"</code>" has no machine-readable mode, the \
                     wizard runs its "<strong class="text-white">"own"</strong>" structured checks to drive each step. \
                     The "<code class="text-blue-400">"flutter doctor -v"</code>" text is captured for display only. \
                     Checks run concurrently and each reports one of five statuses."
                </p>

                <div class="grid grid-cols-2 md:grid-cols-5 gap-2 mt-4 text-center text-xs">
                    <div class="p-2 bg-slate-900 rounded border border-slate-800">
                        <span class="text-green-400 font-bold">"Ok"</span>
                        <p class="text-slate-500 mt-1">"Present & working"</p>
                    </div>
                    <div class="p-2 bg-slate-900 rounded border border-slate-800">
                        <span class="text-amber-400 font-bold">"Partial"</span>
                        <p class="text-slate-500 mt-1">"Present, incomplete"</p>
                    </div>
                    <div class="p-2 bg-slate-900 rounded border border-slate-800">
                        <span class="text-red-400 font-bold">"Missing"</span>
                        <p class="text-slate-500 mt-1">"Not found"</p>
                    </div>
                    <div class="p-2 bg-slate-900 rounded border border-slate-800">
                        <span class="text-orange-400 font-bold">"Error"</span>
                        <p class="text-slate-500 mt-1">"Probe failed"</p>
                    </div>
                    <div class="p-2 bg-slate-900 rounded border border-slate-800">
                        <span class="text-slate-400 font-bold">"Unknown"</span>
                        <p class="text-slate-500 mt-1">"Could not determine"</p>
                    </div>
                </div>

                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Component"</th>
                                <th class="p-4 font-medium">"How it is checked"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <CheckRow comp="Flutter SDK" how="Reuses the 12-strategy SDK locator, then probes flutter --version --machine." />
                            <CheckRow comp="git" how="git --version (required for the default clone install)." />
                            <CheckRow comp="JDK" how="Resolves a JDK home and validates it has bin/java and bin/javac (a full JDK 17+, not a JRE)." />
                            <CheckRow comp="Android cmdline-tools" how="Looks for sdkmanager / avdmanager under cmdline-tools/latest/bin." />
                            <CheckRow comp="Android platform-tools" how="Looks for adb under platform-tools/." />
                            <CheckRow comp="Android platform" how="Looks for any platforms/android-XX SDK image." />
                            <CheckRow comp="Android build-tools" how="Looks for any build-tools/<version>/ install." />
                            <CheckRow comp="Android licenses" how="Checks that the SDK licenses have been accepted." />
                            <CheckRow comp="Prerequisites" how="Per-OS package detection (cmake/ninja/clang on Linux, Xcode CLT on macOS, git on Windows)." />
                        </tbody>
                    </table>
                </div>
            </Section>

            // ── Managed Flutter SDK ───────────────────────────────────
            <Section title="Managed Flutter SDK">
                <p class="text-slate-400">
                    "The Flutter SDK step is fully automated. By default it does a shallow "
                    <code class="text-blue-400">"git clone"</code>" of the configured channel so "
                    <code class="text-blue-400">"flutter upgrade"</code>" and "
                    <code class="text-blue-400">"flutter channel"</code>" keep working; when git is unavailable it \
                     falls back to downloading and verifying a release archive."
                </p>
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"git clone (default)"</h4>
                        <p class="text-sm text-slate-400">
                            <code class="text-green-400">"git clone -b <channel> --depth 1"</code>
                            " into the install root, then "<code class="text-green-400">"flutter precache"</code>
                            " (non-fatal on failure)."
                        </p>
                    </div>
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"archive (fallback)"</h4>
                        <p class="text-sm text-slate-400">
                            "Resolves the arch-correct stable archive + SHA-256 from the Flutter release manifest, \
                             streams the download, verifies the digest, and extracts it."
                        </p>
                    </div>
                </div>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <p class="font-medium mb-1">"Atomic and resumable-friendly"</p>
                    <p>
                        "Installs happen in a temp directory and are atomically renamed into place; a lockfile \
                         guards against concurrent installs, and partial/incomplete prior installs are reclaimed. \
                         The default install root is "<code class="text-blue-300">"~/fvm/versions/<version>"</code>
                        " \u{2014} shared with the "
                        <A href="/docs/installation" attr:class="text-blue-300 hover:underline">"Flutter Version panel\u{2019}s"</A>
                        " FVM cache, so a freshly installed version shows up there too."
                    </p>
                </div>
                <p class="text-slate-400 mt-4">
                    "After a successful install the wizard writes "<code class="text-blue-400">"[flutter] sdk_path"</code>
                    " into "<code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>", so "
                    <strong class="text-white">"fdemon resolves the new SDK immediately"</strong>" \u{2014} no restart \
                     needed for fdemon itself (you still need to restart your terminal for "
                    <code class="text-blue-400">"flutter"</code>" to be on your shell\u{2019}s PATH)."
                </p>
            </Section>

            // ── Android Toolchain ─────────────────────────────────────
            <Section title="Android Toolchain">
                <p class="text-slate-400">
                    "The Android Tools step downloads the command-line tools, performs the required "
                    <code class="text-blue-400">"cmdline-tools/latest/"</code>" relocation, accepts licenses \
                     non-interactively, and runs "<code class="text-blue-400">"sdkmanager"</code>" to install the \
                     packages Flutter needs. It is gated on a present, valid JDK."
                </p>
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Stage"</th>
                                <th class="p-4 font-medium">"Detail"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"cmdline-tools"</td>
                                <td class="p-4 text-slate-300">
                                    "Downloads "<code>"commandlinetools-<os>-<build>_latest.zip"</code>", extracts to \
                                     a temp dir, and relocates it to "<code>"<sdk_root>/cmdline-tools/latest/"</code>"."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"licenses"</td>
                                <td class="p-4 text-slate-300">
                                    "Runs "<code>"sdkmanager --licenses"</code>" non-interactively to accept all SDK \
                                     package licenses (idempotent)."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"packages"</td>
                                <td class="p-4 text-slate-300">
                                    "Installs "<code>"platform-tools"</code>", "<code>"platforms;android-<api>"</code>", "
                                    <code>"build-tools;<api>.0.0"</code>", and "<code>"cmdline-tools;latest"</code>"."
                                </td>
                            </tr>
                            <tr class="hover:bg-slate-900/50">
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"JDK"</td>
                                <td class="p-4 text-slate-300">
                                    "Resolves a JDK from "<code>"jdk_path"</code>", "<code>"$JAVA_HOME"</code>", or "
                                    <code>"which java"</code>"; validates it is a full JDK. Missing JDK \u{2192} a guided \
                                     install command instead of running the step."
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
                <div class="bg-amber-900/20 border border-amber-800 p-4 rounded-lg text-amber-100 text-sm mt-4">
                    <strong>"JDK install is privileged, so it is guided."</strong>
                    " The wizard shows the platform-specific command (apt/dnf, "<code>"brew install"</code>", or "
                    <code>"winget"</code>"). Install it, then press "<code>"r"</code>" to re-check \u{2014} the Android \
                     step unlocks once the JDK check turns green."
                </div>
            </Section>

            // ── PATH & Environment ────────────────────────────────────
            <Section title="PATH & Environment">
                <p class="text-slate-400">
                    "A process can\u{2019}t change its parent shell\u{2019}s environment, so the PATH step writes to your \
                     shell\u{2019}s rc files and asks you to restart your terminal. Writes are "
                    <strong class="text-white">"idempotent and marker-fenced"</strong>" \u{2014} re-running never \
                     duplicates a block, and an outdated block is replaced in place."
                </p>
                <CodeBlock language="bash" code="# >>> fdemon flutter path >>>
export PATH=\"$PATH:/home/you/fvm/versions/stable/bin\"
# <<< fdemon flutter path <<<

# >>> fdemon android env >>>
export ANDROID_HOME='/home/you/.android/sdk'
export PATH=\"$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$ANDROID_HOME/emulator:$PATH\"
# <<< fdemon android env <<<" />
                <div class="overflow-hidden rounded-lg border border-slate-800 mt-4">
                    <table class="w-full text-left text-sm">
                        <thead class="bg-slate-900 text-slate-200">
                            <tr>
                                <th class="p-4 font-medium">"Shell"</th>
                                <th class="p-4 font-medium">"Target"</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-slate-800 bg-slate-950">
                            <CheckRow comp="bash" how="~/.bash_profile (macOS) or ~/.bashrc (Linux)" />
                            <CheckRow comp="zsh" how="~/.zshenv (preferred), falling back to ~/.zprofile" />
                            <CheckRow comp="fish" how="~/.config/fish/config.fish via fish_add_path" />
                            <CheckRow comp="PowerShell / cmd" how="The user PATH in the registry, with a WM_SETTINGCHANGE broadcast" />
                        </tbody>
                    </table>
                </div>
                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Windows re-check without restart:"</strong>
                    " before each preflight on Windows, fdemon refreshes its own process PATH from the registry, \
                     so pressing "<code>"r"</code>" after installing git or another tool (e.g. via winget) finds it \
                     without relaunching fdemon."
                </div>
            </Section>

            // ── Keybindings ───────────────────────────────────────────
            <Section title="Keybindings">
                <p class="text-slate-400">
                    "Open the wizard with "<code class="text-blue-400">"I"</code>" from the log view. The panel has a \
                     two-pane layout \u{2014} the step list on the left and per-step detail on the right \u{2014} and "
                    <code class="text-blue-400">"Tab"</code>" toggles focus between them."
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
                            <KeyRow key="I" action="Open the Install Wizard (from Normal mode); preflight runs automatically" />
                            <KeyRow key="Tab" action="Switch focus between the step list and the detail pane" />
                            <KeyRow key="\u{2191} / k" action="Move up in the step list, or scroll the detail pane up" />
                            <KeyRow key="\u{2193} / j" action="Move down in the step list, or scroll the detail pane down" />
                            <KeyRow key="Enter" action="Run (or retry) the selected step \u{2014} no-op on guided-only steps" />
                            <KeyRow key="r" action="Re-run the preflight check (use after completing a guided step)" />
                            <KeyRow key="c" action="Copy the selected guided command to the clipboard" />
                            <KeyRow key="[ / ]" action="Cycle between guided commands when a step offers more than one" />
                            <KeyRow key="Esc" action="Cancel the running step, or close the wizard when idle" />
                            <KeyRow key="Ctrl+C" action="Force quit Flutter Demon" />
                        </tbody>
                    </table>
                </div>
                <p class="text-slate-400 mt-4 text-sm">
                    "See the "
                    <A href="/docs/keybindings" attr:class="text-blue-400 hover:underline">"Keybindings"</A>
                    " page for the full reference, including the context-dependent behavior of "
                    <code class="text-blue-400">"Esc"</code>" while a step is running."
                </p>
            </Section>

            // ── Configuration ─────────────────────────────────────────
            <Section title="Configuration">
                <p class="text-slate-400">
                    "All wizard behavior is configured under the "<code class="text-blue-400">"[toolchain]"</code>
                    " section of "<code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>
                    ". Every field is optional \u{2014} a missing block uses the defaults shown below."
                </p>
                <CodeBlock language="toml" code="[toolchain]
# Where managed Flutter SDKs are installed (default: ~/fvm/versions/<version>)
# flutter_install_dir = \"~/fvm/versions\"
channel = \"stable\"                  # Flutter channel to install
flutter_install_method = \"git\"      # \"git\" (default) or \"archive\"

# Android SDK root (default: $ANDROID_HOME / $ANDROID_SDK_ROOT, else the per-OS
# default). Written automatically after a successful Android Tools install.
# android_sdk_root = \"~/.android/sdk\"

android_api_level = 36              # Android API level for platforms/build-tools

# cmdline-tools build number for the download URL. Override only if it 404s.
# cmdline_tools_build = \"11076708\"

# Explicit JDK 17 directory, passed to `flutter config --jdk-dir`.
# jdk_path = \"/usr/lib/jvm/java-17-openjdk\"" />
                <SettingsTable entries=vec![
                    ("flutter_install_dir", "string", "~/fvm/versions", "Directory for managed Flutter SDKs; each version goes in a <version> subdirectory. Shared with the Flutter Version panel's FVM cache."),
                    ("channel", "string", "\"stable\"", "Flutter channel to install (\"stable\", \"beta\", \"main\", ...)."),
                    ("flutter_install_method", "string", "\"git\"", "\"git\" clones the Flutter repo (default); \"archive\" downloads a prebuilt archive."),
                    ("android_sdk_root", "string", "$ANDROID_HOME / per-OS default", "Android SDK root override. Written automatically after a successful Android Tools install."),
                    ("android_api_level", "integer", "36", "Android API level for platforms/ and build-tools/ installation."),
                    ("cmdline_tools_build", "string", "(current)", "cmdline-tools build number used in the download URL. Override only if the default 404s."),
                    ("jdk_path", "string", "(auto-detect)", "Explicit JDK 17 directory, passed to flutter config --jdk-dir. Auto-detected from $JAVA_HOME / which java when unset."),
                ] />
                <p class="text-slate-400 mt-4 text-sm">
                    "For the full configuration reference, see the "
                    <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>
                    " page."
                </p>
            </Section>

            // ── Troubleshooting ───────────────────────────────────────
            <Section title="Troubleshooting">
                <div class="space-y-4">
                    <Tip
                        title="Android Tools step won't run?"
                        text="It is gated on a valid JDK 17+. If the JDK check is not green, the wizard shows a \
                               guided install command instead. Install a JDK (or point jdk_path / JAVA_HOME at one), \
                               then press r to re-check."
                    />
                    <Tip
                        title="flutter still not found after install?"
                        text="The PATH step writes to your shell rc files, but a running shell can't pick those up. \
                               Restart your terminal. fdemon itself does not need a restart \u{2014} it writes \
                               [flutter] sdk_path and resolves the new SDK immediately."
                    />
                    <Tip
                        title="cmdline-tools download 404s?"
                        text="Google's commandlinetools URL embeds a build number with no stable alias. Set \
                               cmdline_tools_build in [toolchain] to the current value from \
                               developer.android.com/studio#command-tools, then re-run the step."
                    />
                    <Tip
                        title="A guided (sudo/GUI) step won't auto-run?"
                        text="That's by design. apt/dnf, xcode-select, JDK installs, and Rosetta are privileged or \
                               GUI-driven, so the wizard shows the exact command instead of running sudo for you. \
                               Copy it with c, run it, then press r."
                    />
                    <Tip
                        title="Need to start over?"
                        text="Press r at any time to re-run the full preflight. Each step is independently retriable \
                               with Enter, and a running step can be cancelled with Esc."
                    />
                </div>

                <p class="text-slate-400 mt-4 text-sm">
                    "Related: "
                    <A href="/docs/installation" attr:class="text-blue-400 hover:underline">"Installation"</A>
                    " (installing fdemon itself and the Flutter Version panel), "
                    <A href="/docs/keybindings" attr:class="text-blue-400 hover:underline">"Keybindings"</A>
                    ", and "
                    <A href="/docs/configuration" attr:class="text-blue-400 hover:underline">"Configuration"</A>
                    "."
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
fn CheckRow(comp: &'static str, how: &'static str) -> impl IntoView {
    view! {
        <tr class="hover:bg-slate-900/50 transition-colors">
            <td class="p-4 font-mono text-blue-400 whitespace-nowrap">{comp}</td>
            <td class="p-4 text-slate-300">{how}</td>
        </tr>
    }
}

#[component]
fn SettingsTable(
    entries: Vec<(&'static str, &'static str, &'static str, &'static str)>,
) -> impl IntoView {
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
fn Tip(title: &'static str, text: &'static str) -> impl IntoView {
    view! {
        <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
            <h4 class="font-bold text-white mb-1">{title}</h4>
            <p class="text-sm text-slate-400">{text}</p>
        </div>
    }
}
