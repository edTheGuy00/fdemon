use leptos::prelude::*;

use crate::data::{ChangelogEntry, ChangelogGroup, changelog_entries};

#[component]
pub fn Changelog() -> impl IntoView {
    let entries = changelog_entries();

    view! {
        <div class="animate-fade-in space-y-8">
            <h1 class="text-3xl font-bold text-white">"Changelog"</h1>
            <p class="text-lg text-slate-400">
                "All notable changes to Flutter Demon are documented here. \
                 This project follows semantic versioning."
            </p>

            {entries.into_iter().map(|entry| {
                view! { <VersionEntry entry=entry /> }
            }).collect_view()}
        </div>
    }
}

// ── Version Entry ─────────────────────────────────────────────────────────────

#[component]
fn VersionEntry(entry: ChangelogEntry) -> impl IntoView {
    view! {
        <section class="space-y-6 border border-slate-800 rounded-xl p-6 bg-slate-900/30">
            // Version header
            <div class="flex items-center gap-4">
                <div class="w-2 h-8 bg-blue-500 rounded-full shrink-0"></div>
                <div class="flex items-center gap-3 flex-wrap">
                    <h2 class="text-2xl font-bold text-white">
                        "v"{entry.version}
                    </h2>
                    <span class="inline-flex items-center px-3 py-1 rounded-full text-xs font-medium bg-blue-900/40 text-blue-300 border border-blue-700/50">
                        {entry.version}
                    </span>
                    <span class="text-slate-500 text-sm font-mono">{entry.date}</span>
                </div>
            </div>

            // Change groups
            <div class="space-y-5 pl-5">
                {entry.groups.into_iter().map(|group| {
                    view! { <GroupSection group=group /> }
                }).collect_view()}
            </div>
        </section>
    }
}

// ── Group Section ─────────────────────────────────────────────────────────────

#[component]
fn GroupSection(group: ChangelogGroup) -> impl IntoView {
    let (badge_class, label_class) = group_colors(group.group);

    view! {
        <div class="space-y-3">
            <div class="flex items-center gap-2">
                <span class=badge_class>
                    {group.group}
                </span>
            </div>
            <ul class="space-y-2">
                {group.changes.into_iter().map(|change| {
                    let label_class = label_class.to_string();
                    view! {
                        <li class="flex items-start gap-2 text-sm text-slate-300">
                            <span class="mt-2 w-1.5 h-1.5 rounded-full bg-slate-600 shrink-0"></span>
                            <span>
                                {change.scope.map(|s| view! {
                                    <span class=format!("font-mono text-xs {} mr-1", label_class)>
                                        "("{s}")"
                                    </span>
                                })}
                                {change.description}
                            </span>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </div>
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `(badge_class, label_class)` for a changelog group name.
fn group_colors(group: &str) -> (&'static str, &'static str) {
    match group {
        "Features" => (
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-green-900/40 text-green-300 border border-green-700/50",
            "text-green-400",
        ),
        "Bug Fixes" => (
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-red-900/40 text-red-300 border border-red-700/50",
            "text-red-400",
        ),
        "Documentation" => (
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-blue-900/40 text-blue-300 border border-blue-700/50",
            "text-blue-400",
        ),
        "Performance" => (
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-yellow-900/40 text-yellow-300 border border-yellow-700/50",
            "text-yellow-400",
        ),
        "Refactoring" => (
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-purple-900/40 text-purple-300 border border-purple-700/50",
            "text-purple-400",
        ),
        "Testing" => (
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-cyan-900/40 text-cyan-300 border border-cyan-700/50",
            "text-cyan-400",
        ),
        _ => (
            "inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold bg-slate-800 text-slate-300 border border-slate-700",
            "text-slate-400",
        ),
    }
}
