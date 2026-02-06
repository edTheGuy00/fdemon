use leptos::prelude::*;

use crate::components::icons::Cpu;

#[component]
pub fn Architecture() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-6">
            <h1 class="text-3xl font-bold text-white">"Architecture"</h1>
            <p class="text-slate-400">
                "Flutter Demon is built in Rust and acts as a wrapper around the "
                <code class="text-blue-400">"flutter"</code>
                " tool. It manages multiple child processes (flutter run) and parses their stdout/json streams to update the TUI state."
            </p>
            <div class="p-8 border border-dashed border-slate-700 rounded-xl text-center">
                <Cpu class="w-12 h-12 text-slate-600 mx-auto mb-4" />
                <p class="text-slate-500">"Architecture diagrams coming soon."</p>
            </div>
        </div>
    }
}
