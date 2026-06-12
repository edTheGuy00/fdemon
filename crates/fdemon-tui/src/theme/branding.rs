//! Application branding constants, swapped at compile time by the `pro` feature.
//!
//! The default build brands as "Flutter Demon" with the standard accent color;
//! building with `--features pro` rebrands the title surfaces as "Fdemon PRO"
//! in gold. Keeping both variants here means render code never branches on the
//! feature itself.

use ratatui::style::Color;

#[cfg(not(feature = "pro"))]
pub const APP_TITLE: &str = "Flutter Demon";

#[cfg(feature = "pro")]
pub const APP_TITLE: &str = "Fdemon PRO";

#[cfg(not(feature = "pro"))]
pub const TITLE_COLOR: Color = super::palette::ACCENT;

#[cfg(feature = "pro")]
pub const TITLE_COLOR: Color = Color::Rgb(255, 215, 0);
