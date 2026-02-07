//! File reference extraction for Link Highlight Mode.
//!
//! This module re-exports hyperlink types from the app layer.
//! All state and logic has been moved to app/hyperlinks.rs.

// Re-export all types from app layer
pub use crate::app::hyperlinks::{
    extract_file_ref_from_message, DetectedLink, FileReference, FileReferenceSource,
    LinkHighlightState, MAX_LINK_SHORTCUTS,
};
