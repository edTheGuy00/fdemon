use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::A;

use crate::components::icons::{ChevronRight, Download};
use crate::components::terminal_mockup::TerminalMockup;
use crate::data::features;

#[component]
pub fn Home() -> impl IntoView {
    let feats = features();

    view! {
        <Title text="Flutter Demon — A Rust TUI for Flutter" formatter=|t: String| t />
        <Meta name="description" content="Flutter Demon (fdemon) is a blazingly fast terminal UI for Flutter development written in Rust. Hot reload, multi-device sessions, real-time logs, and DevTools — all from your terminal." />
        <Link rel="canonical" href="https://fdemon.dev/" />
        <div class="space-y-24 pb-24">
            // Hero Section
            <section class="pt-20 px-6 text-center relative overflow-hidden">
                <div class="absolute top-0 left-1/2 -translate-x-1/2 w-[800px] h-[500px] bg-blue-600/10 blur-[120px] rounded-full pointer-events-none"></div>

                <div class="relative z-10 flex flex-col items-center">
                    <img
                        src="/public/logo.png"
                        alt="Flutter Demon logo — a Rust-powered terminal UI for Flutter development"
                        class="w-24 h-24 rounded-2xl mb-8 shadow-lg shadow-blue-500/20"
                    />

                    <h1 class="text-5xl md:text-7xl font-black text-white tracking-tight mb-6">
                        "Flutter "
                        <span class="text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400">
                            "Demon"
                        </span>
                    </h1>

                    <p class="text-xl text-slate-400 max-w-2xl mx-auto mb-4 font-light leading-relaxed">
                        "A blazingly fast flutter terminal ui for Flutter development, written in Rust."
                        <br />
                        "Run apps, view logs, and manage multiple devices — without an IDE."
                    </p>

                    <div class="flex flex-wrap justify-center gap-4 mb-12">
                        <A
                            href="/docs/installation"
                            attr:class="px-8 py-3 bg-white text-black font-bold rounded-full hover:bg-slate-200 transition-colors flex items-center"
                        >
                            <Download class="w-5 h-5 mr-2" />
                            <span>"Install fdemon with Cargo"</span>
                        </A>
                        <A
                            href="/docs"
                            attr:class="px-8 py-3 bg-slate-800 text-white font-bold rounded-full border border-slate-700 hover:bg-slate-700 transition-colors flex items-center"
                        >
                            "Read the Documentation"
                            <ChevronRight class="w-4 h-4 ml-1" />
                        </A>
                    </div>

                    <div class="flex gap-2">
                        <img
                            alt="GitHub Release"
                            src="https://img.shields.io/github/v/release/edTheGuy00/fdemon?style=flat&labelColor=1d1d1d&color=54c5f8&logo=GitHub&logoColor=white"
                            class="h-6"
                        />
                        <img
                            alt="License"
                            src="https://img.shields.io/badge/license-BSL%201.1-white?style=flat&labelColor=1d1d1d"
                            class="h-6"
                        />
                    </div>
                </div>

                <TerminalMockup />
            </section>

            // What is Flutter Demon — descriptive prose
            <section class="px-6 max-w-3xl mx-auto space-y-6 text-slate-400 leading-relaxed">
                <h2 class="text-2xl font-bold text-white">
                    "Flutter Demon — A Rust TUI for Flutter Development"
                </h2>

                <p>
                    "Flutter Demon ("
                    <code class="text-blue-400">"fdemon"</code>
                    ") is a flutter tui that replaces the terminal output you already watch with a \
                     full-screen, keyboard-driven interface. It wraps the Flutter toolchain\u{2019}s \
                     machine-mode protocol and gives you structured, filterable logs, a live status \
                     bar, and one-keystroke hot reload \u{2014} all without opening an IDE. If you \
                     already do your flutter development without an IDE, fdemon gives you the \
                     observability layer your workflow is missing."
                </p>

                <h2 class="text-2xl font-bold text-white">"Hot Reload Without an IDE"</h2>

                <p>
                    "The flutter hot reload cli experience in fdemon is intentional: press "
                    <code class="text-blue-400">"r"</code>
                    " for hot reload, "
                    <code class="text-blue-400">"R"</code>
                    " for hot restart, and "
                    <code class="text-blue-400">"s"</code>
                    " to stop. A smart file watcher monitors your source files \
                     (default: "<code class="text-blue-400">"lib/"</code>", \
                     configurable in "
                    <A
                        href="/docs/configuration"
                        attr:class="text-blue-400 hover:underline"
                    >
                        "your project\u{2019}s configuration"
                    </A>
                    ") and triggers reload on save with intelligent debouncing \u{2014} so you can \
                     keep your hands on the keyboard and your eyes on the logs."
                </p>

                <h2 class="text-2xl font-bold text-white">"Multi-Device Flutter Sessions"</h2>

                <p>
                    "Open the New Session dialog with "
                    <code class="text-blue-400">"+"</code>
                    ", check multiple connected devices with "
                    <code class="text-blue-400">"Space"</code>
                    ", and press Enter to launch them all simultaneously. Each device gets its own \
                     session tab with independent logs, scroll position, and filter state. Up to 9 \
                     sessions can run in parallel, switchable by number key or Tab."
                </p>

                <h2 class="text-2xl font-bold text-white">"Boot Your Whole Stack"</h2>

                <p>
                    "Beyond Flutter output, fdemon can run arbitrary commands as custom log sources. \
                     Define a backend server, a log tailer, or a health-check script in "
                    <A
                        href="/docs/native-logs"
                        attr:class="text-blue-400 hover:underline"
                    >
                        "the Native Logs configuration"
                    </A>
                    " and fdemon starts it alongside your Flutter session. All output \u{2014} Dart \
                     logs, native platform logs, and your custom source \u{2014} is interleaved in \
                     one unified view. One command boots the whole stack."
                </p>

                <h2 class="text-2xl font-bold text-white">"From Bare Machine to Running App"</h2>

                <p>
                    "No Flutter installed? No problem. Press "
                    <code class="text-blue-400">"I"</code>
                    " and the "
                    <A
                        href="/docs/toolchain"
                        attr:class="text-blue-400 hover:underline"
                    >
                        "Install Wizard"
                    </A>
                    " turns \u{201c}Flutter SDK not found\u{201d} into a guided setup: it diagnoses \
                     your toolchain, installs a managed Flutter SDK \u{2014} any version, picked \
                     from a built-in version picker \u{2014} sets up the Android SDK and licenses, \
                     writes your shell PATH, and checks each platform (Android, iOS, macOS, Web, \
                     Windows) with copy-paste commands for anything that needs "
                    <code class="text-blue-400">"sudo"</code>
                    " or a GUI. fdemon never runs "
                    <code class="text-blue-400">"sudo"</code>
                    " for you \u{2014} you stay in control, the wizard re-checks until everything \
                     is green."
                </p>
            </section>

            // Features Grid
            <section class="px-6 max-w-6xl mx-auto">
                <div class="text-center mb-16">
                    <h2 class="text-3xl font-bold text-white mb-4">
                        "Why Flutter Demon?"
                    </h2>
                    <p class="text-slate-400">
                        "Designed for keyboard-first power users \u{2014} mouse support is opt-in."
                    </p>
                </div>

                <div class="grid md:grid-cols-2 gap-8">
                    {feats.into_iter().map(|feat| {
                        view! {
                            <div class="bg-slate-900/50 border border-slate-800 p-8 rounded-2xl hover:border-slate-700 transition-colors">
                                <div class="mb-4 bg-slate-800 w-12 h-12 rounded-lg flex items-center justify-center">
                                    {(feat.icon)()}
                                </div>
                                <h3 class="text-xl font-bold text-white mb-2">{feat.title}</h3>
                                <p class="text-slate-400 leading-relaxed">{feat.desc}</p>
                            </div>
                        }
                    }).collect_view()}
                </div>

                <div class="mt-12 text-center">
                    <p class="text-slate-400">
                        "Get started in minutes \u{2014} "
                        <A
                            href="/docs/installation"
                            attr:class="text-blue-400 hover:underline"
                        >
                            "install fdemon from the releases page"
                        </A>
                        " or build from source with Cargo. See "
                        <A
                            href="/docs/configuration"
                            attr:class="text-blue-400 hover:underline"
                        >
                            "the configuration guide"
                        </A>
                        " to customise watcher paths, key bindings, and DevTools settings. \
                         Check the "
                        <A
                            href="/docs/changelog"
                            attr:class="text-blue-400 hover:underline"
                        >
                            "changelog"
                        </A>
                        " for the latest release notes."
                    </p>
                </div>
            </section>
        </div>
    }
}
