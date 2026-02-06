use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="border-t border-slate-900 bg-slate-950 py-12 mt-auto">
            <div class="max-w-7xl mx-auto px-6 flex flex-col md:flex-row justify-between items-center text-slate-500 text-sm">
                <p>"\u{00A9} 2026 Flutter Demon Contributors. BSL 1.1 License."</p>
                <div class="flex space-x-6 mt-4 md:mt-0">
                    <a href="#" class="hover:text-white transition-colors">"Privacy"</a>
                    <a href="#" class="hover:text-white transition-colors">"Security"</a>
                    <a href="#" class="hover:text-white transition-colors">"Terms"</a>
                </div>
            </div>
        </footer>
    }
}
