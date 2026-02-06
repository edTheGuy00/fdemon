use leptos::prelude::*;

use crate::data::{dialog_keybindings, normal_keybindings, settings_keybindings, Keybinding};

#[component]
pub fn Keybindings() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Keyboard Bindings"</h1>
            <p class="text-slate-400">
                "Flutter Demon provides extensive keyboard controls for efficient terminal-based development. \
                 Here is a complete reference of all bindings."
            </p>

            <KeybindingSection
                title="Normal Mode"
                color="bg-blue-500"
                key_color="text-blue-400"
                bindings=normal_keybindings()
            />

            <KeybindingSection
                title="New Session Dialog"
                color="bg-green-500"
                key_color="text-green-400"
                bindings=dialog_keybindings()
            />

            <KeybindingSection
                title="Settings"
                color="bg-purple-500"
                key_color="text-purple-400"
                bindings=settings_keybindings()
            />
        </div>
    }
}

#[component]
fn KeybindingSection(
    title: &'static str,
    color: &'static str,
    key_color: &'static str,
    bindings: Vec<Keybinding>,
) -> impl IntoView {
    let indicator_class = format!("w-2 h-6 {color} mr-3 rounded-full");

    view! {
        <section>
            <h2 class="text-xl font-bold text-white mb-4 flex items-center">
                <div class=indicator_class></div>
                {title}
            </h2>
            <div class="overflow-hidden rounded-lg border border-slate-800">
                <table class="w-full text-left text-sm">
                    <thead class="bg-slate-900 text-slate-200">
                        <tr>
                            <th class="p-4 font-medium">"Key"</th>
                            <th class="p-4 font-medium">"Action"</th>
                            <th class="p-4 font-medium hidden md:table-cell">"Description"</th>
                        </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-800 bg-slate-950">
                        {bindings.into_iter().map(|bind| {
                            let kc = key_color.to_string();
                            let key_class = format!("p-4 font-mono {kc} whitespace-nowrap");
                            view! {
                                <tr class="hover:bg-slate-900/50 transition-colors">
                                    <td class=key_class>{bind.key}</td>
                                    <td class="p-4 text-white font-medium">{bind.action}</td>
                                    <td class="p-4 text-slate-500 hidden md:table-cell">{bind.description}</td>
                                </tr>
                            }
                        }).collect_view()}
                    </tbody>
                </table>
            </div>
        </section>
    }
}
