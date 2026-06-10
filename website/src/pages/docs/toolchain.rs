use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::A;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Toolchain() -> impl IntoView {
    view! {
        <Title text="Install Wizard" />
        <Meta name="description" content="Flutter Demon's guided Install Wizard diagnoses a missing or incomplete Flutter toolchain and installs what it can \u{2014} a managed Flutter SDK, the Android command-line tools, JDK detection, and shell PATH configuration. It also detects and guides iOS/macOS (Xcode, CocoaPods), Web (browser), and Windows (Visual Studio C++) platform requirements, across Linux, macOS, and Windows." />
        <Link rel="canonical" href="https://fdemon.dev/docs/toolchain" />
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Install Wizard"</h1>
            <p class="text-lg text-slate-400">
                "On a fresh machine, \u{201c}Flutter SDK not found\u{201d} is a dead end. The Install Wizard \
                 replaces that one-line error with a guided, "<code class="text-blue-400">"flutter doctor"</code>
                "-style screen that diagnoses exactly what is missing and walks you through fixing it \
                 \u{2014} a managed Flutter SDK, the Android command-line tools, a JDK, and your shell PATH. \
                 It also diagnoses iOS/macOS (Xcode, CocoaPods), Web (browser), and Windows \
                 (Visual Studio C++) platform requirements and shows you the exact copy-paste commands \
                 for anything it cannot install automatically."
            </p>

            // ── Overview ──────────────────────────────────────────────
            <Section title="Overview">
                <p class="text-slate-400">
                    "Press "<code class="text-blue-400">"I"</code>" from the log view to open the wizard. It runs a \
                     toolchain preflight \u{2014} fdemon\u{2019}s own structured checks plus, when Flutter is present, \
                     the embedded "<code class="text-blue-400">"flutter doctor -v"</code>" output \u{2014} and presents \
                     five steps in order, each with a roll-up status."
                </p>
                <div class="bg-amber-900/20 border border-amber-800 p-4 rounded-lg text-amber-100 text-sm mt-2">
                    <strong>"Hybrid by design:"</strong>
                    " safe, self-contained steps (download/extract the Flutter SDK, fetch the Android \
                     cmdline-tools, run "<code>"sdkmanager"</code>", write your shell rc files) are run \
                     automatically. Steps that need "<code>"sudo"</code>" or a GUI (apt/dnf, "
                    <code>"xcode-select --install"</code>", JDK install, Rosetta, Visual Studio Installer) are "
                    <strong>"never auto-run"</strong>" \u{2014} the wizard shows the exact copy-paste command and \
                     re-checks when you\u{2019}re done. fdemon never runs "<code>"sudo"</code>" for you."
                </div>
                <div class="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-1">"Diagnose"</h4>
                        <p class="text-sm text-slate-400">
                            "A structured preflight reports Flutter, git, JDK, Android cmdline-tools, \
                             platform-tools, platforms/build-tools, licenses, OS prerequisites, web browser, \
                             Xcode/CocoaPods (macOS), and Visual Studio C++ (Windows)."
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
                    "The wizard\u{2019}s left pane lists five steps; the right pane shows per-step \
                     detail \u{2014} the underlying component checks, guided commands, and live install progress. \
                     Each step shows a roll-up status (OK / Partial / Missing / Pending). Step 2 \
                     ("<strong class="text-white">"Platforms"</strong>") is an expandable submenu: press "
                    <code class="text-blue-400">"Enter"</code>" on it to reveal or hide the per-platform \
                     leaves (Android, Web, iOS, macOS, Windows)."
                </p>

                <div class="bg-slate-900 rounded-lg border border-slate-800 p-4 font-mono text-xs text-slate-400 overflow-x-auto mt-4">
                    <pre class="leading-relaxed">{"\
\u{250c}\u{2500} Install Wizard \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2510}
\u{2502} Steps                 \u{2502} Platforms                              \u{2502}
\u{2502} \u{2714} Prerequisites      \u{2502}   [\u{2714}] Android                           \u{2502}
\u{2502} \u{25bc} Platforms          \u{2502}   [\u{25cb}] Web  (no browser found)           \u{2502}
\u{2502}   \u{2714}  Android         \u{2502}                                        \u{2502}
\u{2502}   \u{25cb}  Web             \u{2502}   c: copy  r: re-check                 \u{2502}
\u{2502}   \u{25cb}  iOS             \u{2502}   [ / ]: cycle commands                \u{2502}
\u{2502}   \u{25cb}  macOS           \u{2502}                                        \u{2502}
\u{2502} \u{2714} Flutter SDK        \u{2502}                                        \u{2502}
\u{2502} \u{25cb} PATH Config        \u{2502}                                        \u{2502}
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
                                <td class="p-4 font-mono text-blue-400 whitespace-nowrap">"2. Platforms"</td>
                                <td class="p-4 text-slate-300 whitespace-nowrap">"Mixed (per leaf)"</td>
                                <td class="p-4 text-slate-300 hidden md:table-cell">
                                    "Expandable submenu (press "<code>"Enter"</code>") with per-platform leaves. "
                                    <strong>"Android"</strong>" \u{2014} auto-installs cmdline-tools, runs "
                                    <code>"sdkmanager"</code>", accepts licenses; JDK-gated. "
                                    <strong>"Web"</strong>", "<strong>"iOS"</strong>", "<strong>"macOS"</strong>
                                    ", "<strong>"Windows"</strong>" \u{2014} detect only; show guided copy-paste commands \
                                     when something is missing. Host-gated: iOS/macOS leaves appear on macOS, \
                                     Windows leaf appears on Windows. Non-blocking: a missing browser or absent \
                                     Xcode/VS never blocks the wizard."
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
                    " the UI order mirrors the dependency order (prerequisites \u{2192} platforms \u{2192} \
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
                            <CheckRow comp="Web browser" how="Checks CHROME_EXECUTABLE, then common install paths for Chrome and Chromium; on Windows also probes for Edge. All hosts." />
                            <CheckRow comp="Xcode / CocoaPods" how="macOS only. Checks that full Xcode (not just CLT) is selected via xcode-select -p and that xcodebuild -version succeeds. Separately checks for the CocoaPods gem." />
                            <CheckRow comp="Visual Studio C++" how="Windows only. Two-gate vswhere query: gate 1 detects any VS instance; gate 2 additionally requires the VC.Tools.x86.x64 and VC.CMake.Project components. Status capped at Partial (non-blocking)." />
                        </tbody>
                    </table>
                </div>

                <div class="bg-blue-900/20 border border-blue-800 p-4 rounded-lg text-blue-200 text-sm mt-4">
                    <strong>"Non-blocking platform leaves:"</strong>
                    " Web, iOS/macOS, and Windows leaves cap their status at "
                    <strong>"Partial"</strong>" \u{2014} they never surface "<strong>"Missing"</strong>" to the \
                     Platforms parent. A machine without Chrome or Xcode is not broken; the wizard still \
                     installs Flutter and the Android toolchain normally."
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

            // ── Android Platform Leaf ─────────────────────────────────
            <Section title="Android Platform Leaf">
                <p class="text-slate-400">
                    "The Android leaf under Platforms downloads the command-line tools, performs the required "
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
                     leaf unlocks once the JDK check turns green."
                </div>
            </Section>

            // ── Guided-Only Platform Leaves ───────────────────────────
            <Section title="Guided-Only Platform Leaves">
                <p class="text-slate-400">
                    "Web, iOS, macOS, and Windows are "<strong class="text-white">"detect-and-guide"</strong>" leaves: \
                     the wizard probes each platform\u{2019}s tooling, and when something is absent it shows \
                     copy-paste commands \u{2014} it never auto-runs a browser install, Xcode, or Visual Studio. \
                     These leaves are "<strong class="text-white">"non-blocking"</strong>": their status is capped \
                     at Partial, so a missing browser or absent Xcode never stalls the wizard. Press "
                    <code class="text-blue-400">"r"</code>" after completing a guided step to re-check; "
                    <code class="text-blue-400">"c"</code>" copies the shown command; "
                    <code class="text-blue-400">"["</code>" / "<code class="text-blue-400">"]"</code>
                    " cycle through multiple commands."
                </p>

                <div class="space-y-4 mt-4">
                    // Web
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-2">"Web (all hosts)"</h4>
                        <p class="text-sm text-slate-400 mb-3">
                            "Checks "<code class="text-blue-400">"CHROME_EXECUTABLE"</code>", then common install \
                             paths for Chrome and Chromium (on Windows, also Microsoft Edge). If no browser is \
                             found it shows per-OS install hints. On Linux, a package-manager hint \
                             (e.g. "<code>"sudo apt install chromium-browser"</code>") is shown alongside a \
                             download-URL fallback. On macOS and Windows a direct-download URL is shown."
                        </p>
                        <p class="text-sm text-slate-500">
                            "To point Flutter at a non-default browser, set "
                            <code class="text-blue-400">"CHROME_EXECUTABLE"</code>" in your shell or in "
                            <code class="text-blue-400">"[toolchain] web_browser_executable"</code>" in "
                            <code class="text-blue-400 bg-slate-800 px-1 rounded">".fdemon/config.toml"</code>
                            " (see Configuration below)."
                        </p>
                    </div>

                    // iOS / macOS
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-2">"iOS & macOS (macOS host only)"</h4>
                        <p class="text-sm text-slate-400 mb-2">
                            "Both leaves require full "<strong class="text-white">"Xcode"</strong>" (not just \
                             Command Line Tools) selected via "<code class="text-blue-400">"xcode-select"</code>
                            " and "<strong class="text-white">"CocoaPods"</strong>". When absent, the wizard shows:"
                        </p>
                        <div class="overflow-hidden rounded-lg border border-slate-700">
                            <table class="w-full text-left text-xs">
                                <tbody class="divide-y divide-slate-700 bg-slate-950">
                                    <tr class="hover:bg-slate-900/50">
                                        <td class="p-3 font-mono text-blue-400 whitespace-nowrap">"Install Xcode"</td>
                                        <td class="p-3 text-slate-400">
                                            <code>"open \"https://apps.apple.com/us/app/xcode/id497799835\""</code>
                                            " (or "<code>"brew install --cask xcodes"</code>")."
                                        </td>
                                    </tr>
                                    <tr class="hover:bg-slate-900/50">
                                        <td class="p-3 font-mono text-blue-400 whitespace-nowrap">"Select Xcode & accept license"</td>
                                        <td class="p-3 text-slate-400">
                                            <code>"sudo xcode-select -s /Applications/Xcode.app/Contents/Developer && sudo xcodebuild -runFirstLaunch && sudo xcodebuild -license accept"</code>
                                        </td>
                                    </tr>
                                    <tr class="hover:bg-slate-900/50">
                                        <td class="p-3 font-mono text-blue-400 whitespace-nowrap">"Download the iOS platform (iOS only)"</td>
                                        <td class="p-3 text-slate-400">
                                            <code>"xcodebuild -downloadPlatform iOS"</code>
                                        </td>
                                    </tr>
                                    <tr class="hover:bg-slate-900/50">
                                        <td class="p-3 font-mono text-blue-400 whitespace-nowrap">"Install CocoaPods"</td>
                                        <td class="p-3 text-slate-400">
                                            <code>"brew install cocoapods"</code>
                                            " (or "<code>"sudo gem install cocoapods"</code>")."
                                        </td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </div>

                    // Windows
                    <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                        <h4 class="font-bold text-white mb-2">"Windows (Windows host only)"</h4>
                        <p class="text-sm text-slate-400 mb-2">
                            "Detects Visual Studio with the \u{201c}Desktop development with C++\u{201d} workload via a \
                             two-gate "<code class="text-blue-400">"vswhere"</code>" query. When VS is present but \
                             the C++ workload is missing, an additional \u{201c}Modify\u{201d} entry is shown first. \
                             Fresh-install options follow:"
                        </p>
                        <div class="overflow-hidden rounded-lg border border-slate-700">
                            <table class="w-full text-left text-xs">
                                <tbody class="divide-y divide-slate-700 bg-slate-950">
                                    <tr class="hover:bg-slate-900/50">
                                        <td class="p-3 font-mono text-blue-400 whitespace-nowrap">"Modify existing VS (if VS found)"</td>
                                        <td class="p-3 text-slate-400">
                                            "Opens the Visual Studio Installer via "
                                            <code>r#"start "" "%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\setup.exe""#</code>
                                            " \u{2014} choose Modify and tick \u{201c}Desktop development with C++\u{201d}."
                                        </td>
                                    </tr>
                                    <tr class="hover:bg-slate-900/50">
                                        <td class="p-3 font-mono text-blue-400 whitespace-nowrap">"Install VS Build Tools (winget)"</td>
                                        <td class="p-3 text-slate-400">
                                            <code>"winget install --id Microsoft.VisualStudio.2022.BuildTools --override \"--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.NativeDesktop;includeRecommended\""</code>
                                            " (shown when winget is available)."
                                        </td>
                                    </tr>
                                    <tr class="hover:bg-slate-900/50">
                                        <td class="p-3 font-mono text-blue-400 whitespace-nowrap">"Install VS Build Tools (choco)"</td>
                                        <td class="p-3 text-slate-400">
                                            <code>"choco install visualstudio2022buildtools --package-parameters \"--add Microsoft.VisualStudio.Workload.NativeDesktop --includeRecommended\""</code>
                                        </td>
                                    </tr>
                                </tbody>
                            </table>
                        </div>
                    </div>
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
                            <KeyRow key="Enter" action="On the Platforms row: expand or collapse the platform submenu. On an auto-install step: run (or retry) the step. No-op on guided-only leaves." />
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
# default). Written automatically after a successful Android platform install.
# android_sdk_root = \"~/.android/sdk\"

android_api_level = 36              # Android API level for platforms/build-tools

# cmdline-tools build number for the download URL. Override only if it 404s.
# cmdline_tools_build = \"11076708\"

# Explicit JDK 17 directory, passed to `flutter config --jdk-dir`.
# jdk_path = \"/usr/lib/jvm/java-17-openjdk\"

# Explicit path to a Chromium-based browser for `flutter run -d chrome`.
# Overrides CHROME_EXECUTABLE and the default search paths.
# web_browser_executable = \"/usr/bin/chromium\"" />
                <SettingsTable entries=vec![
                    ("flutter_install_dir", "string", "~/fvm/versions", "Directory for managed Flutter SDKs; each version goes in a <version> subdirectory. Shared with the Flutter Version panel's FVM cache."),
                    ("channel", "string", "\"stable\"", "Flutter channel to install (\"stable\", \"beta\", \"main\", ...)."),
                    ("flutter_install_method", "string", "\"git\"", "\"git\" clones the Flutter repo (default); \"archive\" downloads a prebuilt archive."),
                    ("android_sdk_root", "string", "$ANDROID_HOME / per-OS default", "Android SDK root override. Written automatically after a successful Android platform leaf install."),
                    ("android_api_level", "integer", "36", "Android API level for platforms/ and build-tools/ installation."),
                    ("cmdline_tools_build", "string", "(current)", "cmdline-tools build number used in the download URL. Override only if the default 404s."),
                    ("jdk_path", "string", "(auto-detect)", "Explicit JDK 17 directory, passed to flutter config --jdk-dir. Auto-detected from $JAVA_HOME / which java when unset."),
                    ("web_browser_executable", "string", "(auto-detect)", "Explicit path to a Chromium-based browser for flutter run -d chrome. Overrides CHROME_EXECUTABLE and the default search paths (Chrome, Chromium, Edge)."),
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
                        title="Android leaf won't run?"
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
                        text="That's by design. apt/dnf, Xcode, JDK installs, Visual Studio, and Rosetta are \
                               privileged or GUI-driven, so the wizard shows the exact command instead of running \
                               sudo for you. Copy it with c, run it, then press r."
                    />
                    <Tip
                        title="Web leaf shows a browser warning but I have Chrome installed?"
                        text="Set web_browser_executable in [toolchain] to the absolute path of your browser \
                               binary, or export CHROME_EXECUTABLE before launching fdemon. The Web leaf is \
                               non-blocking and never stalls Flutter or Android setup."
                    />
                    <Tip
                        title="iOS/macOS leaf is Partial but I have Xcode installed?"
                        text="The wizard checks for full Xcode (not just Command Line Tools) selected via \
                               xcode-select. Run: sudo xcode-select -s /Applications/Xcode.app/Contents/Developer \
                               && sudo xcodebuild -runFirstLaunch && sudo xcodebuild -license accept \
                               then press r to re-check."
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
