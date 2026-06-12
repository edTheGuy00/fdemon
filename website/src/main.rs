use flutter_demon_website::App;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).expect("error initializing logger");
    mount_to_body(App);
    remove_splash();
}

/// Remove the static splash screen defined in `index.html`.
///
/// The app is CSR-mounted (`mount_to_body` *appends* to `<body>`), so the
/// splash overlay painted while the WASM bundle was downloading must be
/// explicitly removed once the component tree has mounted.
fn remove_splash() {
    if let Some(splash) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("splash"))
    {
        splash.remove();
    }
}
