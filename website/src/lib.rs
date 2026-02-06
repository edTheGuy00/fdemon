pub mod components;
pub mod data;
pub mod pages;

use components::footer::Footer;
use components::navbar::Navbar;
use leptos::prelude::*;
use leptos_router::components::{ParentRoute, Route, Router, Routes};
use leptos_router::hooks::use_location;
use leptos_router::path;
use pages::docs::architecture::Architecture;
use pages::docs::configuration::Configuration;
use pages::docs::installation::Installation;
use pages::docs::introduction::Introduction;
use pages::docs::keybindings::Keybindings;
use pages::docs::DocsLayout;
use pages::home::Home;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <ScrollToTop />
            <div class="min-h-screen bg-slate-950 text-slate-200 selection:bg-blue-500/30 font-sans">
                <Navbar />
                <Routes fallback=|| "Page not found.">
                    <Route path=path!("/") view=Home />
                    <ParentRoute path=path!("/docs") view=DocsLayout>
                        <Route path=path!("/") view=Introduction />
                        <Route path=path!("/installation") view=Installation />
                        <Route path=path!("/keybindings") view=Keybindings />
                        <Route path=path!("/configuration") view=Configuration />
                        <Route path=path!("/architecture") view=Architecture />
                    </ParentRoute>
                </Routes>
                <Footer />
            </div>
        </Router>
    }
}

#[component]
fn ScrollToTop() -> impl IntoView {
    let location = use_location();

    Effect::new(move || {
        let _ = location.pathname.get();
        if let Some(window) = web_sys::window() {
            window.scroll_to_with_x_and_y(0.0, 0.0);
        }
    });

    ()
}
