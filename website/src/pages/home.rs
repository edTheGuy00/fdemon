use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::icons::{ChevronRight, Download};
use crate::components::terminal_mockup::TerminalMockup;
use crate::data::features;

#[component]
pub fn Home() -> impl IntoView {
    let feats = features();

    view! {
        <div class="space-y-24 pb-24">
            // Hero Section
            <section class="pt-20 px-6 text-center relative overflow-hidden">
                <div class="absolute top-0 left-1/2 -translate-x-1/2 w-[800px] h-[500px] bg-blue-600/10 blur-[120px] rounded-full pointer-events-none"></div>

                <div class="relative z-10 flex flex-col items-center">
                    <img src="/public/logo.png" alt="Flutter Demon" class="w-24 h-24 rounded-2xl mb-8 shadow-lg shadow-blue-500/20" />

                    <h1 class="text-5xl md:text-7xl font-black text-white tracking-tight mb-6">
                        "Flutter "
                        <span class="text-transparent bg-clip-text bg-gradient-to-r from-blue-400 to-purple-400">
                            "Demon"
                        </span>
                    </h1>

                    <p class="text-xl text-slate-400 max-w-2xl mx-auto mb-8 font-light leading-relaxed">
                        "A blazingly fast TUI for Flutter development."
                        <br />
                        "Run apps, view logs, and manage multiple devices from your terminal."
                    </p>

                    <div class="flex flex-wrap justify-center gap-4 mb-12">
                        <A
                            href="/docs/installation"
                            attr:class="px-8 py-3 bg-white text-black font-bold rounded-full hover:bg-slate-200 transition-colors flex items-center"
                        >
                            <Download class="w-5 h-5 mr-2" />
                            "Install Now"
                        </A>
                        <A
                            href="/docs"
                            attr:class="px-8 py-3 bg-slate-800 text-white font-bold rounded-full border border-slate-700 hover:bg-slate-700 transition-colors flex items-center"
                        >
                            "Read Docs"
                            <ChevronRight class="w-4 h-4 ml-1" />
                        </A>
                    </div>

                    <div class="flex gap-2">
                        <img
                            alt="Release"
                            src="https://img.shields.io/badge/release-v0.1.0-blue?style=flat&labelColor=1d1d1d&color=54c5f8"
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

            // Features Grid
            <section class="px-6 max-w-6xl mx-auto">
                <div class="text-center mb-16">
                    <h2 class="text-3xl font-bold text-white mb-4">
                        "Why Flutter Demon?"
                    </h2>
                    <p class="text-slate-400">
                        "Designed for power users who prefer the keyboard over the mouse."
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
            </section>
        </div>
    }
}
