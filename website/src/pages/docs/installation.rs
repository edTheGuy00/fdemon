use leptos::prelude::*;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Installation() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-6">
            <h1 class="text-3xl font-bold text-white">"Installation"</h1>
            <div class="bg-yellow-900/20 border border-yellow-800 p-4 rounded-lg text-yellow-200 text-sm">
                <strong>"Note:"</strong>
                " Pre-built binaries for Windows, macOS, and Linux are coming soon!"
            </div>

            <h2 class="text-xl font-bold text-white mt-8">"Build from Source"</h2>
            <p class="text-slate-400">"Requirements: Rust 1.70+, Flutter SDK"</p>
            <CodeBlock code="# Clone the repository\ngit clone https://github.com/edTheGuy00/flutter-demon.git\ncd flutter-demon\n\n# Build\ncargo build --release\n\n# Run\n./target/release/fdemon" />
        </div>
    }
}
