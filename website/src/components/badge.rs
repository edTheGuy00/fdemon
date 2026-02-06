use leptos::prelude::*;

#[derive(Clone, Copy, Default)]
pub enum BadgeColor {
    #[default]
    Blue,
    Green,
    Orange,
}

impl BadgeColor {
    fn class(self) -> &'static str {
        match self {
            BadgeColor::Blue => "px-2 py-0.5 text-xs font-medium rounded border bg-blue-900/30 text-blue-300 border-blue-800",
            BadgeColor::Green => "px-2 py-0.5 text-xs font-medium rounded border bg-green-900/30 text-green-300 border-green-800",
            BadgeColor::Orange => "px-2 py-0.5 text-xs font-medium rounded border bg-orange-900/30 text-orange-300 border-orange-800",
        }
    }
}

#[component]
pub fn Badge(
    children: Children,
    #[prop(optional)] color: BadgeColor,
) -> impl IntoView {
    view! {
        <span class=color.class()>
            {children()}
        </span>
    }
}
