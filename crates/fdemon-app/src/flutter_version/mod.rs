//! # Flutter Version Panel State
//!
//! State and types for the Flutter Version panel (opened with `V`),
//! which displays the current SDK info and installed versions.
//!
//! This module mirrors the `new_session_dialog/` pattern:
//! - `state.rs`  — `FlutterVersionState`, `SdkInfoState`, `VersionListState`
//! - `types.rs`  — `FlutterVersionPane`, `InstalledSdk`

mod state;
mod types;

pub use state::*;
pub use types::*;
