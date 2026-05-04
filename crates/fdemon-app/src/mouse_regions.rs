//! Per-frame mouse click-region registry.
//!
//! Widgets push entries during render; the [`crate::handler::mouse`] dispatcher
//! reads entries on click. The registry lives on [`crate::state::AppState`] as
//! a `Cell<MouseRegions>` (TEA exception, see `docs/CODE_STANDARDS.md`
//! Principle 3 — same exception class as the existing `Cell<usize>` render-hint
//! feedback).
//!
//! ## Lifecycle
//!
//! 1. `render::view()` calls `state.mouse_regions.take()` to drain the previous
//!    frame, then constructs a [`MouseRegionsBuilder`].
//! 2. Widgets that need clickable surfaces call `ctx.click(rect, action)` /
//!    `ctx.click_with(rect, button, action)` during their `render()` method.
//! 3. After all widgets have rendered, `render::view()` puts the populated
//!    [`MouseRegions`] back via `state.mouse_regions.set(regions)`.
//! 4. On `Message::Mouse(MouseInput::Press { x, y, button, .. })`, the handler
//!    layer calls `regions.hit_test(x, y, button)` to find the matching region.

// The public API of this module is intentionally unused until Task 03 wires
// `MouseRegions` into `AppState` and Task 04+ adds widget call sites.
#![allow(dead_code)]

use crate::input_mouse::MouseButton;
use crate::message::Message;

/// Rectangle in terminal cell coordinates. Mirrors `ratatui::layout::Rect`
/// without the dependency — `fdemon-app` does not depend on `ratatui`.
///
/// The TUI side defines `impl From<ratatui::layout::Rect> for MouseRect` to
/// make conversion at the boundary a one-liner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MouseRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl MouseRect {
    /// Convenience constructor.
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns `true` when `(px, py)` falls inside the rect (left/top inclusive,
    /// right/bottom exclusive — matching ratatui's hit-test convention).
    pub const fn contains(self, px: u16, py: u16) -> bool {
        px >= self.x && py >= self.y && (px - self.x) < self.width && (py - self.y) < self.height
    }

    /// Returns `true` when the rect has zero width or zero height. Hit-tests
    /// always return `false` for empty rects; widgets should skip pushing
    /// empty rects.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// What to do when a region is clicked.
///
/// `Emit` boxes its `Message` payload so that `MouseAction` stays pointer-sized
/// rather than inheriting the full size of the largest `Message` variant (~352 B).
#[derive(Debug, Clone)]
pub enum MouseAction {
    /// Emit a single [`Message`] verbatim.
    Emit(Box<Message>),
    /// Emit a [`Message`] computed from the click coordinates.
    /// Used in Phase 4+ for log-row clicks, frame-bar clicks, etc.
    EmitWithCoord(fn(u16, u16) -> Message),
}

impl MouseAction {
    /// Convenience constructor: wraps `msg` in a `Box` and constructs `Emit`.
    pub fn emit(msg: Message) -> Self {
        MouseAction::Emit(Box::new(msg))
    }

    /// Resolve the action into a concrete [`Message`] for the click at
    /// `(x, y)`. `Emit` ignores the coordinate; `EmitWithCoord` invokes the
    /// stored function.
    pub fn resolve(&self, x: u16, y: u16) -> Message {
        match self {
            MouseAction::Emit(msg) => *msg.clone(),
            MouseAction::EmitWithCoord(f) => f(x, y),
        }
    }
}

/// Single click-region entry.
#[derive(Debug, Clone)]
pub struct MouseRegionEntry {
    pub rect: MouseRect,
    /// Action for left button press. `None` = unbound.
    pub on_left: Option<MouseAction>,
    /// Action for middle button press. `None` = unbound.
    pub on_middle: Option<MouseAction>,
    /// Higher z-index wins on overlap. Modal layers (Phase 5: dialogs,
    /// overlays) record at `z_index = 1`; everything else uses `0`.
    pub z_index: u8,
}

/// Per-frame click-region registry.
///
/// Backing storage: `Vec<MouseRegionEntry>`. The vec is reused frame-over-frame
/// via `Cell::take` + `Vec::clear` (no realloc) to keep the renderer hot path
/// allocation-free at steady state.
#[derive(Debug, Default)]
pub struct MouseRegions {
    entries: Vec<MouseRegionEntry>,
}

impl MouseRegions {
    /// Pre-sized constructor (allocates capacity for 32 entries — covers the
    /// worst case of header + 9 tabs + 9 device rows + 6 settings rows).
    pub fn with_capacity() -> Self {
        Self {
            entries: Vec::with_capacity(32),
        }
    }

    /// Find the highest-`z_index` entry whose rect contains `(x, y)` and
    /// has a binding for `button`. Ties on `z_index` are broken by
    /// last-pushed-wins (later entries shadow earlier ones at the same z),
    /// because widgets pushed later in render order are drawn on top.
    ///
    /// Implementation: enumerate entries in registration order and use a
    /// composite key `(z_index, push_index)` so that higher z wins, and
    /// among ties in z the higher push index (last-pushed) wins.
    pub fn hit_test(&self, x: u16, y: u16, button: MouseButton) -> Option<&MouseRegionEntry> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.rect.contains(x, y))
            .filter(|(_, e)| match button {
                MouseButton::Left => e.on_left.is_some(),
                MouseButton::Middle => e.on_middle.is_some(),
                MouseButton::Right => false, // reserved for future
            })
            .max_by_key(|(i, e)| (e.z_index, *i))
            .map(|(_, e)| e)
    }

    /// Return a builder borrowed against this registry. The builder appends
    /// entries to the existing vec; `Vec::clear` is the caller's responsibility
    /// (typically `render::view()` calls `clear()` before handing the builder
    /// to widgets).
    pub fn builder(&mut self) -> MouseRegionsBuilder<'_> {
        MouseRegionsBuilder { regions: self }
    }

    /// Drop all entries while preserving the backing vec's capacity.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of registered entries (useful for tests).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when no entries are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterator over registered entries in registration order.
    pub fn iter(&self) -> std::slice::Iter<'_, MouseRegionEntry> {
        self.entries.iter()
    }
}

/// Borrowed builder used during render to push click regions.
#[derive(Debug)]
pub struct MouseRegionsBuilder<'a> {
    regions: &'a mut MouseRegions,
}

impl<'a> MouseRegionsBuilder<'a> {
    /// Register a left-click-only region at `z_index = 0`.
    pub fn click(&mut self, rect: MouseRect, action: MouseAction) {
        if rect.is_empty() {
            return;
        }
        self.regions.entries.push(MouseRegionEntry {
            rect,
            on_left: Some(action),
            on_middle: None,
            z_index: 0,
        });
    }

    /// Register a left-click-only region at a specific `z_index`. Phase 5
    /// dialogs/overlays use `z_index = 1`; Phase 3 widgets stay at `0`.
    pub fn click_at_z(&mut self, rect: MouseRect, action: MouseAction, z_index: u8) {
        if rect.is_empty() {
            return;
        }
        self.regions.entries.push(MouseRegionEntry {
            rect,
            on_left: Some(action),
            on_middle: None,
            z_index,
        });
    }

    /// Register a region with separate left and middle bindings (used for
    /// session tabs: left = select, middle = close).
    pub fn click_left_middle(
        &mut self,
        rect: MouseRect,
        on_left: MouseAction,
        on_middle: MouseAction,
    ) {
        if rect.is_empty() {
            return;
        }
        self.regions.entries.push(MouseRegionEntry {
            rect,
            on_left: Some(on_left),
            on_middle: Some(on_middle),
            z_index: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    #[test]
    fn rect_contains_inclusive_top_left_exclusive_bottom_right() {
        let r = MouseRect::new(5, 10, 3, 2);
        assert!(r.contains(5, 10)); // top-left
        assert!(r.contains(7, 11)); // last cell inside (x=5+3-1, y=10+2-1)
        assert!(!r.contains(8, 10)); // exclusive right edge
        assert!(!r.contains(5, 12)); // exclusive bottom edge
        assert!(!r.contains(4, 10)); // outside left
        assert!(!r.contains(5, 9)); // outside top
    }

    #[test]
    fn empty_rect_contains_nothing() {
        assert!(!MouseRect::new(0, 0, 0, 5).contains(0, 0));
        assert!(!MouseRect::new(0, 0, 5, 0).contains(0, 0));
    }

    #[test]
    fn builder_skips_empty_rects() {
        let mut regions = MouseRegions::default();
        regions.builder().click(
            MouseRect::new(0, 0, 0, 5),
            MouseAction::emit(Message::HotReload),
        );
        assert!(regions.is_empty());
    }

    #[test]
    fn click_with_left_only_does_not_match_middle() {
        let mut regions = MouseRegions::default();
        regions.builder().click(
            MouseRect::new(0, 0, 4, 1),
            MouseAction::emit(Message::HotReload),
        );
        assert!(regions.hit_test(1, 0, MouseButton::Left).is_some());
        assert!(regions.hit_test(1, 0, MouseButton::Middle).is_none());
        assert!(regions.hit_test(1, 0, MouseButton::Right).is_none());
    }

    #[test]
    fn click_left_middle_binds_both_buttons() {
        let mut regions = MouseRegions::default();
        regions.builder().click_left_middle(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SelectSessionByIndex(0)),
            // TODO: switch to Message::CloseSessionAt(0) when Task 02 lands.
            MouseAction::emit(Message::CloseCurrentSession),
        );
        let left = regions.hit_test(0, 0, MouseButton::Left).unwrap();
        let middle = regions.hit_test(0, 0, MouseButton::Middle).unwrap();
        assert!(matches!(left.on_left, Some(MouseAction::Emit(_))));
        assert!(matches!(middle.on_middle, Some(MouseAction::Emit(_))));
    }

    #[test]
    fn higher_z_index_wins_on_overlap() {
        let mut regions = MouseRegions::default();
        regions.builder().click_at_z(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::HotReload),
            0,
        );
        regions.builder().click_at_z(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::RequestQuit),
            1,
        );
        let hit = regions.hit_test(5, 0, MouseButton::Left).unwrap();
        // Box<Message> pattern: match through the box with a ref guard.
        assert!(matches!(
            &hit.on_left,
            Some(MouseAction::Emit(msg)) if matches!(**msg, Message::RequestQuit)
        ));
    }

    #[test]
    fn last_pushed_wins_at_same_z() {
        // Two regions at z=0 overlap; the second push (later widget paint order)
        // should be returned by hit_test.
        let mut regions = MouseRegions::default();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::HotReload),
        );
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::HotRestart),
        );
        let hit = regions.hit_test(0, 0, MouseButton::Left).unwrap();
        // Box<Message> pattern: match through the box with a ref guard.
        assert!(matches!(
            &hit.on_left,
            Some(MouseAction::Emit(msg)) if matches!(**msg, Message::HotRestart)
        ));
    }

    #[test]
    fn click_outside_all_regions_returns_none() {
        let mut regions = MouseRegions::default();
        regions.builder().click(
            MouseRect::new(0, 0, 4, 1),
            MouseAction::emit(Message::HotReload),
        );
        assert!(regions.hit_test(100, 100, MouseButton::Left).is_none());
    }

    #[test]
    fn emit_with_coord_resolves_to_coordinate_message() {
        let action = MouseAction::EmitWithCoord(|_x, y| Message::SelectSessionByIndex(y as usize));
        assert!(matches!(
            action.resolve(0, 4),
            Message::SelectSessionByIndex(4)
        ));
    }

    #[test]
    fn clear_preserves_capacity() {
        let mut regions = MouseRegions::with_capacity();
        let cap_before = regions.entries.capacity();
        for i in 0..16 {
            regions.builder().click(
                MouseRect::new(i, 0, 1, 1),
                MouseAction::emit(Message::HotReload),
            );
        }
        regions.clear();
        assert!(regions.is_empty());
        assert!(regions.entries.capacity() >= cap_before);
    }

    #[test]
    fn right_button_never_matches() {
        let mut regions = MouseRegions::default();
        regions.builder().click_left_middle(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::HotReload),
            MouseAction::emit(Message::HotReload),
        );
        assert!(regions.hit_test(0, 0, MouseButton::Right).is_none());
    }
}
