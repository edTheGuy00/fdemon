use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use super::icons::Github;

#[component]
pub fn Navbar() -> impl IntoView {
    let location = use_location();
    let pathname = move || location.pathname.get();

    let is_home = move || pathname() == "/";
    let is_docs = move || pathname().starts_with("/docs");

    view! {
        <header class="fixed top-0 left-0 right-0 z-50 bg-slate-950/80 backdrop-blur-md border-b border-slate-800">
            <div class="max-w-7xl mx-auto px-6 h-16 flex items-center justify-between">
                <A href="/" attr:class="flex items-center space-x-2 group">
                    <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-600 to-purple-600 flex items-center justify-center text-lg group-hover:scale-110 transition-transform">
                        "\u{1F608}"
                    </div>
                    <span class="font-bold text-white text-lg tracking-tight">
                        "Flutter Demon"
                    </span>
                </A>

                <nav class="hidden md:flex items-center space-x-8">
                    <A
                        href="/"
                        attr:class=move || {
                            if is_home() {
                                "text-sm font-medium transition-colors text-white"
                            } else {
                                "text-sm font-medium transition-colors text-slate-400 hover:text-white"
                            }
                        }
                    >
                        "Home"
                    </A>
                    <A
                        href="/docs"
                        attr:class=move || {
                            if is_docs() {
                                "text-sm font-medium transition-colors text-white"
                            } else {
                                "text-sm font-medium transition-colors text-slate-400 hover:text-white"
                            }
                        }
                    >
                        "Documentation"
                    </A>
                    <a
                        href="https://github.com/edTheGuy00/flutter-demon"
                        target="_blank"
                        rel="noreferrer"
                        class="text-slate-400 hover:text-white transition-colors"
                    >
                        <Github class="w-5 h-5" />
                    </a>
                </nav>
            </div>
        </header>
    }
}
