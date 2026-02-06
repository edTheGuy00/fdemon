use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use super::icons::{Check, Copy};

#[component]
pub fn CodeBlock(
    code: &'static str,
    #[prop(default = "bash")] language: &'static str,
) -> impl IntoView {
    let (copied, set_copied) = signal(false);

    let handle_copy = move |_| {
        let code = code.to_string();
        leptos::task::spawn_local(async move {
            let window = web_sys::window().unwrap();
            let nav = window.navigator();
            let clipboard = nav.clipboard();
            let promise = clipboard.write_text(&code);
            let _ = JsFuture::from(promise).await;
            set_copied.set(true);
            // Reset after 2 seconds
            let cb = wasm_bindgen::closure::Closure::once(move || {
                set_copied.set(false);
            });
            window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    cb.as_ref().unchecked_ref(),
                    2000,
                )
                .unwrap();
            cb.forget();
        });
    };

    view! {
        <div class="relative group rounded-lg overflow-hidden bg-slate-900 border border-slate-800 my-4">
            <div class="flex justify-between items-center px-4 py-2 bg-slate-800/50 border-b border-slate-800">
                <span class="text-xs text-slate-400 font-mono">{language}</span>
                <button
                    on:click=handle_copy
                    class="text-slate-400 hover:text-white transition-colors"
                >
                    {move || {
                        if copied.get() {
                            view! { <Check class="w-4 h-4 text-green-400" /> }.into_any()
                        } else {
                            view! { <Copy class="w-4 h-4" /> }.into_any()
                        }
                    }}
                </button>
            </div>
            <div class="p-4 overflow-x-auto">
                <pre class="text-sm font-mono text-slate-300">{code}</pre>
            </div>
        </div>
    }
}
