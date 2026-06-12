//! Centralized theme system for the Cyber-Glass TUI design.
//!
//! This module provides:
//! - `branding` — App title and title color (compile-time `pro` feature swap)
//! - `palette` — Raw color constants
//! - `styles` — Semantic style builder functions
//! - `icons` — `IconSet` for runtime icon resolution (Unicode/Nerd Font)

pub mod branding;
pub mod icons;
pub mod palette;
pub mod styles;
