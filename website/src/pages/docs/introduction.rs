use leptos::prelude::*;

#[component]
pub fn Introduction() -> impl IntoView {
    view! {
        <div class="animate-fade-in space-y-6">
            <h1 class="text-4xl font-bold text-white">"Flutter Demon"</h1>
            <p class="text-lg text-slate-400">
                "Flutter Demon is a high-performance terminal user interface for Flutter development. \
                 Run your Flutter apps, view logs in real-time, hot reload on file changes, and manage \
                 multiple device sessions \u{2014} all from the comfort of your terminal!"
            </p>

            <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-8">
                <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                    <h3 class="font-bold text-white mb-2">"Efficient Workflow"</h3>
                    <p class="text-sm text-slate-400">
                        "Keep your hands on the keyboard. Toggle sessions, reload, and restart without lifting a finger."
                    </p>
                </div>
                <div class="p-4 bg-slate-900 rounded-lg border border-slate-800">
                    <h3 class="font-bold text-white mb-2">"Resource Friendly"</h3>
                    <p class="text-sm text-slate-400">
                        "Uses significantly less memory than full GUI IDEs. Perfect for laptops and low-resource environments."
                    </p>
                </div>
            </div>
        </div>
    }
}
