use leptos::prelude::*;

use crate::components::code_block::CodeBlock;

#[component]
pub fn Configuration() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-6">
            <h1 class="text-3xl font-bold text-white">"Configuration"</h1>
            <p class="text-slate-400">
                "Flutter Demon supports TOML configuration files. The global config file is located at "
                <code class="text-blue-400 bg-slate-900 px-1 rounded">".fdemon/config.toml"</code>
                "."
            </p>

            <h3 class="text-lg font-bold text-white mt-4">"Global Settings Example"</h3>
            <CodeBlock
                language="toml"
                code="[behavior]\nauto_start = false\nconfirm_quit = true\n\n[watcher]\npaths = [\"lib\"]\ndebounce_ms = 500\nauto_reload = true\n\n[editor]\ncommand = \"\"  # Auto-detect from environment"
            />
        </div>
    }
}
