pub mod architecture;
pub mod configuration;
pub mod installation;
pub mod introduction;
pub mod keybindings;

use leptos::prelude::*;
use leptos_router::components::{A, Outlet};
use leptos_router::hooks::use_location;

use crate::components::icons::{Cpu, Download, FileText, Keyboard, Menu, Settings};

struct DocItem {
    href: &'static str,
    label: &'static str,
    icon: fn() -> AnyView,
}

fn doc_items() -> Vec<DocItem> {
    vec![
        DocItem {
            href: "/docs",
            label: "Introduction",
            icon: || view! { <FileText class="w-4 h-4 mr-3" /> }.into_any(),
        },
        DocItem {
            href: "/docs/installation",
            label: "Installation",
            icon: || view! { <Download class="w-4 h-4 mr-3" /> }.into_any(),
        },
        DocItem {
            href: "/docs/keybindings",
            label: "Keybindings",
            icon: || view! { <Keyboard class="w-4 h-4 mr-3" /> }.into_any(),
        },
        DocItem {
            href: "/docs/configuration",
            label: "Configuration",
            icon: || view! { <Settings class="w-4 h-4 mr-3" /> }.into_any(),
        },
        DocItem {
            href: "/docs/architecture",
            label: "Architecture",
            icon: || view! { <Cpu class="w-4 h-4 mr-3" /> }.into_any(),
        },
    ]
}

#[component]
pub fn DocsLayout() -> impl IntoView {
    let (is_menu_open, set_is_menu_open) = signal(false);
    let location = use_location();
    let pathname = move || location.pathname.get();

    let items = doc_items();

    // Find current page label for mobile header
    let mobile_label = {
        let path = pathname();
        items
            .iter()
            .find(|i| i.href == path)
            .map(|i| i.label)
            .unwrap_or("Menu")
            .to_string()
    };

    view! {
        <div class="flex flex-col md:flex-row min-h-screen pt-16">
            // Mobile Menu Toggle
            <div class="md:hidden p-4 border-b border-slate-800 bg-slate-950 sticky top-16 z-20">
                <button
                    on:click=move |_| set_is_menu_open.update(|v| *v = !*v)
                    class="flex items-center text-slate-300"
                >
                    <Menu class="w-5 h-5 mr-2" />
                    {mobile_label}
                </button>
            </div>

            // Sidebar
            <aside class=move || {
                let transform = if is_menu_open.get() {
                    "translate-x-0"
                } else {
                    "-translate-x-full"
                };
                format!(
                    "fixed md:sticky top-16 left-0 h-[calc(100vh-4rem)] w-64 bg-slate-950 border-r border-slate-800 \
                     shrink-0 self-start \
                     transform {transform} md:translate-x-0 transition-transform duration-200 z-30 \
                     overflow-y-auto"
                )
            }>
                <nav class="p-4 space-y-1">
                    {items.into_iter().map(|item| {
                        let href = item.href;
                        let label = item.label;
                        view! {
                            <A
                                href=href
                                attr:class=move || {
                                    let active = pathname() == href;
                                    if active {
                                        "w-full flex items-center px-4 py-3 text-sm rounded-lg transition-colors bg-blue-900/20 text-blue-400 font-medium"
                                    } else {
                                        "w-full flex items-center px-4 py-3 text-sm rounded-lg transition-colors text-slate-400 hover:bg-slate-900 hover:text-white"
                                    }
                                }
                                on:click=move |_| set_is_menu_open.set(false)
                            >
                                {(item.icon)()}
                                {label}
                            </A>
                        }
                    }).collect_view()}
                </nav>
            </aside>

            // Main Content
            <main class="flex-1 p-6 md:p-12 md:max-w-4xl mx-auto w-full">
                <Outlet />
            </main>
        </div>
    }
}
