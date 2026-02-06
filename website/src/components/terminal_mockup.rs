use leptos::prelude::*;

#[component]
pub fn TerminalMockup() -> impl IntoView {
    view! {
        <div class="w-full max-w-4xl mx-auto mt-12 rounded-xl overflow-hidden shadow-2xl border border-slate-700 bg-[#0f0f12] font-mono text-sm relative z-10">
            // Window Header (OS Chrome)
            <div class="bg-slate-800 px-4 py-2 flex items-center space-x-2 border-b border-slate-700">
                <div class="w-3 h-3 rounded-full bg-red-500/80"></div>
                <div class="w-3 h-3 rounded-full bg-yellow-500/80"></div>
                <div class="w-3 h-3 rounded-full bg-green-500/80"></div>
                <div class="flex-1 text-center text-slate-400 text-xs">
                    "flutter-demon \u{2014} 80x24"
                </div>
            </div>

            // TUI Content
            <div class="p-4 h-96 flex flex-col text-slate-300 font-mono text-xs md:text-sm bg-[#1e1e1e] leading-relaxed">
                // TUI Header Row
                <div class="flex justify-between items-center text-cyan-400 mb-1 font-bold tracking-tight">
                    <div class="flex space-x-2">
                        <span>"Flutter Demon"</span>
                        <span class="text-slate-600">"|"</span>
                        <span class="text-white">"my_app"</span>
                    </div>
                    <div class="text-slate-500 font-normal">
                        <span class="text-yellow-500">"[r]"</span>" "
                        <span class="text-yellow-500">"[R]"</span>" "
                        <span class="text-red-500">"[x]"</span>" "
                        <span class="text-blue-500">"[d]"</span>" "
                        <span class="text-slate-500">"[q]"</span>
                    </div>
                </div>

                // Device Row
                <div class="flex items-center space-x-2 text-green-400 mb-2 border-b border-slate-700/50 pb-2">
                    <span class="text-[10px]">"\u{25CF}"</span>
                    <span class="text-white font-medium">"iPad mini (A17 Pro)"</span>
                </div>

                // Logs Area
                <div class="flex-1 overflow-hidden relative font-mono text-[11px] md:text-xs">
                    <div class="absolute inset-0 overflow-y-auto space-y-0.5 scrollbar-hide pb-2">
                        <LogLine time="00:47:26" tag="app" tag_color="text-purple-400" msg_color="text-green-400" message="Reloaded in 52ms" />

                        <div class="mt-2">
                            <LogLine time="00:47:40" tag="watch" tag_color="text-cyan-400" msg_color="text-white" message="File change detected, reloading..." />
                        </div>
                        <LogLine time="00:47:40" tag="flutter" tag_color="text-blue-400" msg_color="text-green-400" message="Reloaded 5 of 1675 libraries in 214ms" detail="(compile: 31 ms, reload: 102 ms)" />
                        <LogLine time="00:47:40" tag="app" tag_color="text-purple-400" msg_color="text-green-400" message="Reloaded in 258ms" />

                        <div class="mt-2">
                            <LogLine time="00:48:02" tag="watch" tag_color="text-cyan-400" msg_color="text-white" message="File change detected, reloading..." />
                        </div>
                        <LogLine time="00:48:03" tag="flutter" tag_color="text-blue-400" msg_color="text-green-400" message="Reloaded 1 of 1675 libraries in 235ms" detail="(compile: 32 ms, reload: 112 ms)" />
                        <LogLine time="00:48:03" tag="app" tag_color="text-purple-400" msg_color="text-green-400" message="Reloaded in 257ms" />

                        <div class="mt-2">
                            <LogLine time="00:55:01" tag="flutter" tag_color="text-blue-400" msg_color="text-white" message="-[WFIsolatedShortcutRunner init] Taking sandbox extensions for exec" />
                        </div>
                        <LogLine time="00:55:01" tag="flutter" tag_color="text-blue-400" msg_color="text-white" message="Indexing for request: <WFToolKitIndexingRequest: 0x600001720e80>" />
                        <LogLine time="00:55:02" tag="flutter" tag_color="text-blue-400" msg_color="text-white" message="Resolved Preferred localizations: [BackgroundShortcutRunner.ToolKit]" />
                    </div>

                    // Scrollbar Mockup
                    <div class="absolute right-0 top-0 bottom-0 w-1 bg-slate-800/50 rounded-full">
                        <div class="h-1/3 w-full bg-slate-500/80 rounded-full mt-10"></div>
                    </div>
                </div>

                // Footer Status Bar
                <div class="mt-2 border-t border-slate-700/50 pt-1 flex justify-between text-[10px] md:text-xs font-medium">
                    <div class="flex items-center space-x-3 md:space-x-4">
                        <div class="flex items-center text-green-400">
                            <span class="mr-1.5 text-[8px]">"\u{25CF}"</span>" Running"
                        </div>
                        <div class="w-px h-3 bg-slate-700"></div>
                        <div class="text-green-400">"Debug (develop)"</div>
                        <div class="w-px h-3 bg-slate-700"></div>
                        <div class="text-slate-400 font-mono">"01:14:28"</div>
                        <div class="w-px h-3 bg-slate-700 hidden sm:block"></div>
                        <div class="text-slate-400 hidden sm:block font-mono">"00:49:16"</div>
                    </div>

                    <div class="flex items-center space-x-3 md:space-x-4">
                        <div class="w-px h-3 bg-slate-700"></div>
                        <div class="text-red-400">"\u{2717} 20 errors"</div>
                        <div class="w-px h-3 bg-slate-700"></div>
                        <div class="text-green-400 flex items-center">
                            <span class="mr-1">"\u{2B07}"</span>" Auto "
                            <span class="text-slate-500 ml-2 font-mono">"4454-4484/448"</span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[component]
fn LogLine(
    time: &'static str,
    tag: &'static str,
    tag_color: &'static str,
    msg_color: &'static str,
    message: &'static str,
    #[prop(optional)] detail: &'static str,
) -> impl IntoView {
    let tag_class = format!("{tag_color} shrink-0");
    let msg_class = format!("{msg_color} break-all");
    let tag_label = format!("[{tag}]");
    let time_label = format!("{time} \u{2022}");

    view! {
        <div class="flex space-x-2">
            <span class="text-slate-500 shrink-0">{time_label}</span>
            <span class=tag_class>{tag_label}</span>
            <span class=msg_class>
                {message}
                {(!detail.is_empty()).then(|| view! {
                    " "<span class="text-slate-500">{detail}</span>
                })}
            </span>
        </div>
    }
}
