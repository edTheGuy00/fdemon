use leptos::prelude::*;

use crate::data::{all_keybinding_sections, KeybindingSection as KbSection};

#[component]
pub fn Keybindings() -> impl IntoView {
    let sections = all_keybinding_sections();

    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Keyboard Bindings"</h1>
            <p class="text-slate-400">
                "Flutter Demon provides extensive keyboard controls for efficient terminal-based development. \
                 Here is a complete reference of all bindings organized by mode."
            </p>

            {sections.into_iter().map(|section| {
                view! { <KeybindingSectionView section=section /> }
            }).collect_view()}
        </div>
    }
}

#[component]
fn KeybindingSectionView(section: KbSection) -> impl IntoView {
    let indicator_class = format!("w-2 h-6 {} mr-3 rounded-full", section.color);
    let key_color = section.key_color.to_string();

    view! {
        <section>
            <h2 class="text-xl font-bold text-white mb-4 flex items-center">
                <div class=indicator_class></div>
                {section.title}
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
                        {section.bindings.into_iter().map(|bind| {
                            let kc = key_color.clone();
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
