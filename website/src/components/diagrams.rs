use leptos::prelude::*;

/// Color variants for diagram nodes.
/// Full Tailwind class strings in match arms ensure the scanner picks them up.
#[derive(Clone, Copy, Default)]
pub enum NodeColor {
    #[default]
    Blue,
    Green,
    Purple,
    Orange,
    Cyan,
    Slate,
    Yellow,
    Rose,
}

impl NodeColor {
    fn border_class(self) -> &'static str {
        match self {
            Self::Blue => "border-l-blue-500",
            Self::Green => "border-l-green-500",
            Self::Purple => "border-l-purple-500",
            Self::Orange => "border-l-orange-500",
            Self::Cyan => "border-l-cyan-500",
            Self::Slate => "border-l-slate-600",
            Self::Yellow => "border-l-yellow-500",
            Self::Rose => "border-l-rose-500",
        }
    }

    fn bg_class(self) -> &'static str {
        match self {
            Self::Blue => "bg-blue-950/40",
            Self::Green => "bg-green-950/40",
            Self::Purple => "bg-purple-950/40",
            Self::Orange => "bg-orange-950/40",
            Self::Cyan => "bg-cyan-950/40",
            Self::Slate => "bg-slate-900",
            Self::Yellow => "bg-yellow-950/40",
            Self::Rose => "bg-rose-950/40",
        }
    }

    fn title_class(self) -> &'static str {
        match self {
            Self::Blue => "text-blue-400",
            Self::Green => "text-green-400",
            Self::Purple => "text-purple-400",
            Self::Orange => "text-orange-400",
            Self::Cyan => "text-cyan-400",
            Self::Slate => "text-slate-300",
            Self::Yellow => "text-yellow-400",
            Self::Rose => "text-rose-400",
        }
    }

    fn ring_class(self) -> &'static str {
        match self {
            Self::Blue => "ring-blue-500/20",
            Self::Green => "ring-green-500/20",
            Self::Purple => "ring-purple-500/20",
            Self::Orange => "ring-orange-500/20",
            Self::Cyan => "ring-cyan-500/20",
            Self::Slate => "ring-slate-500/20",
            Self::Yellow => "ring-yellow-500/20",
            Self::Rose => "ring-rose-500/20",
        }
    }

    pub fn step_class(self) -> &'static str {
        match self {
            Self::Blue => "bg-blue-500/20 text-blue-400",
            Self::Green => "bg-green-500/20 text-green-400",
            Self::Purple => "bg-purple-500/20 text-purple-400",
            Self::Orange => "bg-orange-500/20 text-orange-400",
            Self::Cyan => "bg-cyan-500/20 text-cyan-400",
            Self::Slate => "bg-slate-500/20 text-slate-400",
            Self::Yellow => "bg-yellow-500/20 text-yellow-400",
            Self::Rose => "bg-rose-500/20 text-rose-400",
        }
    }
}

/// A styled diagram node with colored left border and subtle ring glow.
#[component]
pub fn ArchNode(
    title: &'static str,
    #[prop(optional)] subtitle: &'static str,
    color: NodeColor,
    #[prop(optional)] icon: Option<fn() -> AnyView>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    let border = color.border_class();
    let bg = color.bg_class();
    let title_color = color.title_class();
    let ring = color.ring_class();

    let container = format!(
        "p-3 md:p-4 rounded-lg border border-slate-800 border-l-4 {border} {bg} ring-1 {ring} {class}"
    );
    let title_cls = format!("font-bold text-sm {title_color}");

    view! {
        <div class=container>
            <div class="flex items-center gap-2">
                {icon.map(|f| f())}
                <h4 class=title_cls>{title}</h4>
            </div>
            {(!subtitle.is_empty()).then(|| view! {
                <p class="text-xs text-slate-500 mt-1">{subtitle}</p>
            })}
        </div>
    }
}

/// Vertical arrow connector with optional label.
#[component]
pub fn FlowArrow(#[prop(optional)] label: &'static str) -> impl IntoView {
    if label.is_empty() {
        view! {
            <div class="flex flex-col items-center py-1">
                <div class="w-px h-6 bg-gradient-to-b from-slate-600 to-slate-700"></div>
                <span class="text-slate-600 text-xs leading-none">"▼"</span>
            </div>
        }
        .into_any()
    } else {
        view! {
            <div class="flex flex-col items-center py-1">
                <div class="w-px h-3 bg-gradient-to-b from-slate-600 to-slate-700"></div>
                <span class="text-[10px] text-slate-500 px-2">{label}</span>
                <div class="w-px h-3 bg-gradient-to-b from-slate-600 to-slate-700"></div>
                <span class="text-slate-600 text-xs leading-none">"▼"</span>
            </div>
        }
        .into_any()
    }
}

/// Branch connector: one input splitting to three outputs.
#[component]
pub fn BranchDown3() -> impl IntoView {
    view! {
        <div class="py-1">
            <div class="mx-auto w-px h-5 bg-slate-600"></div>
            <div class="mx-[12%] md:mx-[16%] h-px bg-slate-600"></div>
            <div class="grid grid-cols-3 mx-[12%] md:mx-[16%]">
                <div class="flex flex-col items-center">
                    <div class="w-px h-5 bg-slate-600"></div>
                    <span class="text-slate-600 text-xs leading-none">"▼"</span>
                </div>
                <div class="flex flex-col items-center">
                    <div class="w-px h-5 bg-slate-600"></div>
                    <span class="text-slate-600 text-xs leading-none">"▼"</span>
                </div>
                <div class="flex flex-col items-center">
                    <div class="w-px h-5 bg-slate-600"></div>
                    <span class="text-slate-600 text-xs leading-none">"▼"</span>
                </div>
            </div>
        </div>
    }
}

/// Numbered flow step with colored badge.
#[component]
pub fn FlowStep(
    n: u32,
    title: &'static str,
    desc: &'static str,
    #[prop(optional)] color: NodeColor,
) -> impl IntoView {
    let step = color.step_class();
    let badge = format!(
        "w-7 h-7 rounded-full {step} text-xs font-bold flex items-center justify-center shrink-0"
    );
    let label = format!("{n}");

    view! {
        <div class="flex items-start gap-3 py-2">
            <div class=badge>{label}</div>
            <div class="min-w-0">
                <p class="text-sm text-white font-medium">{title}</p>
                <p class="text-xs text-slate-500 mt-0.5">{desc}</p>
            </div>
        </div>
    }
}

/// Wrapper for a complete diagram with title bar and border.
#[component]
pub fn DiagramContainer(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-slate-800 bg-slate-950/50 overflow-hidden">
            <div class="px-4 py-2.5 bg-slate-900/50 border-b border-slate-800">
                <span class="text-xs font-medium text-slate-400 uppercase tracking-wider">{title}</span>
            </div>
            <div class="p-4 md:p-6">
                {children()}
            </div>
        </div>
    }
}
