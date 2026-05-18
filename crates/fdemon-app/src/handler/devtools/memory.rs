//! Memory panel handlers.
//!
//! Mirrors `handler::devtools::performance` but routes to `session.memory`.
//! Handles allocation profile updates, alloc-table sort/row selection,
//! memory chart scroll, and Tab cycling between Memory subsections.

use super::scroll_helpers::{clamp_chart_scroll, ScrollDir};
use crate::handler::UpdateResult;
use crate::session::memory::{AllocationSortColumn, MemorySection, MemoryState};
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::performance::{AllocationProfile, MemorySample};

// ── Scroll helpers ────────────────────────────────────────────────────────────

/// Fallback page size when the render-hint visible dimension is 0 (not yet rendered).
const DEFAULT_MEM_PAGE_SIZE: usize = 10;

// ── Public handlers ───────────────────────────────────────────────────────────

/// Handle rich memory sample received from the VM service.
///
/// Pushes the sample into `MemoryState::memory_samples` for the session
/// identified by `session_id`. No-op if the session does not exist.
pub(crate) fn handle_memory_sample_received(
    state: &mut AppState,
    session_id: SessionId,
    sample: MemorySample,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.memory.memory_samples.push(sample);
        handle.session.memory.monitoring_active = true;
    }
    UpdateResult::none()
}

/// Handle allocation profile snapshot received from the VM service.
///
/// Replaces `MemoryState::allocation_profile` with the new snapshot for
/// the session identified by `session_id`. Only the most recent profile is
/// retained in state. No-op if the session does not exist.
pub(crate) fn handle_allocation_profile_received(
    state: &mut AppState,
    session_id: SessionId,
    profile: AllocationProfile,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        tracing::debug!(
            "Allocation profile received for session {}: {} classes",
            session_id,
            profile.members.len(),
        );
        handle.session.memory.allocation_profile = Some(profile);
    }
    UpdateResult::none()
}

/// Toggle the allocation table sort between [`AllocationSortColumn::BySize`]
/// and [`AllocationSortColumn::ByInstances`].
///
/// No-op when no session is selected.
pub(crate) fn handle_toggle_allocation_sort(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.memory.allocation_sort = match handle.session.memory.allocation_sort {
            AllocationSortColumn::BySize => AllocationSortColumn::ByInstances,
            AllocationSortColumn::ByInstances => AllocationSortColumn::BySize,
        };
    }
    UpdateResult::none()
}

/// Move keyboard focus to the given sub-section within the Memory panel.
///
/// Sets `memory.focused_section = section`. No-op when no session is selected.
pub(crate) fn handle_mem_focus_section(
    state: &mut AppState,
    section: MemorySection,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.memory.focused_section = section;
    }
    UpdateResult::none()
}

/// Scroll the focused Memory panel section by one row/sample in `direction`.
///
/// Dispatch table:
/// - `Chart` — adjusts `memory_chart_scroll_offset`.
/// - `AllocationList` — adjusts `alloc_table_selected_row`.
///
/// No-op when no session is selected.
pub(crate) fn handle_mem_scroll(state: &mut AppState, direction: ScrollDir) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.memory.focused_section {
        MemorySection::Chart => {
            let buf_len = handle.session.memory.memory_samples.len();
            let visible = handle.session.memory.memory_chart_visible_width.get();
            let delta: i64 = match direction {
                ScrollDir::Up => 1,
                ScrollDir::Down => -1,
            };
            handle.session.memory.memory_chart_scroll_offset = clamp_chart_scroll(
                buf_len,
                visible,
                handle.session.memory.memory_chart_scroll_offset,
                delta,
            );
        }
        MemorySection::AllocationList => {
            scroll_alloc_table(&mut handle.session.memory, direction, 1);
        }
    }

    UpdateResult::none()
}

/// Scroll the focused Memory panel section by one page in `direction`.
///
/// Page size is taken from the appropriate render hint (`memory_chart_visible_width`
/// or `alloc_table_visible_height`); falls back to [`DEFAULT_MEM_PAGE_SIZE`]
/// when the hint is 0 (not yet rendered).
///
/// No-op when no session is selected.
pub(crate) fn handle_mem_page(state: &mut AppState, direction: ScrollDir) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.memory.focused_section {
        MemorySection::Chart => {
            let visible = handle.session.memory.memory_chart_visible_width.get();
            let page = if visible == 0 {
                DEFAULT_MEM_PAGE_SIZE
            } else {
                visible
            } as i64;
            let buf_len = handle.session.memory.memory_samples.len();
            let delta: i64 = match direction {
                ScrollDir::Up => page,
                ScrollDir::Down => -page,
            };
            handle.session.memory.memory_chart_scroll_offset = clamp_chart_scroll(
                buf_len,
                visible,
                handle.session.memory.memory_chart_scroll_offset,
                delta,
            );
        }
        MemorySection::AllocationList => {
            let page = {
                let h = handle.session.memory.alloc_table_visible_height.get();
                if h == 0 {
                    DEFAULT_MEM_PAGE_SIZE
                } else {
                    h
                }
            };
            scroll_alloc_table(&mut handle.session.memory, direction, page);
        }
    }

    UpdateResult::none()
}

/// Jump to the furthest-back position in the focused section (oldest data / first row).
///
/// - `Chart`: set scroll offset to `max_back` (oldest data visible).
/// - `AllocationList`: select row 0 and reset scroll offset.
///
/// No-op when no session is selected.
pub(crate) fn handle_mem_jump_to_start(state: &mut AppState) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.memory.focused_section {
        MemorySection::Chart => {
            let buf_len = handle.session.memory.memory_samples.len();
            let visible = handle
                .session
                .memory
                .memory_chart_visible_width
                .get()
                .max(1);
            handle.session.memory.memory_chart_scroll_offset = buf_len.saturating_sub(visible);
        }
        MemorySection::AllocationList => {
            let row_count = alloc_row_count(&handle.session.memory);
            if row_count > 0 {
                handle.session.memory.alloc_table_selected_row = Some(0);
            } else {
                handle.session.memory.alloc_table_selected_row = None;
            }
            handle.session.memory.alloc_table_scroll_offset = 0;
        }
    }

    UpdateResult::none()
}

/// Jump to the live edge in the focused section (newest data / last row).
///
/// - `Chart`: set scroll offset to 0 (live edge).
/// - `AllocationList`: select the last row and scroll to show it.
///
/// No-op when no session is selected.
pub(crate) fn handle_mem_jump_to_end(state: &mut AppState) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.memory.focused_section {
        MemorySection::Chart => {
            handle.session.memory.memory_chart_scroll_offset = 0;
        }
        MemorySection::AllocationList => {
            let row_count = alloc_row_count(&handle.session.memory);
            if row_count > 0 {
                handle.session.memory.alloc_table_selected_row = Some(row_count - 1);
                let visible = {
                    let h = handle.session.memory.alloc_table_visible_height.get();
                    if h == 0 {
                        DEFAULT_MEM_PAGE_SIZE
                    } else {
                        h
                    }
                };
                handle.session.memory.alloc_table_scroll_offset = row_count.saturating_sub(visible);
            } else {
                handle.session.memory.alloc_table_selected_row = None;
            }
        }
    }

    UpdateResult::none()
}

/// Select a row in the allocation table by index, or clear the selection when `index` is `None`.
///
/// When `index` is `Some(_)`, also sets `memory.focused_section = AllocationList` so the
/// panel focus follows the selection.
///
/// When `index` is `None`, only clears the selection; focus is intentionally left unchanged.
///
/// No-op when no session is selected.
pub(crate) fn handle_mem_select_alloc_row(
    state: &mut AppState,
    index: Option<usize>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.memory.alloc_table_selected_row = index;
        if index.is_some() {
            handle.session.memory.focused_section = MemorySection::AllocationList;
        }
    }
    UpdateResult::none()
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Return the number of rows in the allocation table for the given `MemoryState`.
fn alloc_row_count(mem: &MemoryState) -> usize {
    mem.allocation_profile
        .as_ref()
        .map(|p| p.members.len())
        .unwrap_or(0)
}

/// Scroll the allocation table selection by `steps` rows in `direction`,
/// adjusting the scroll offset to keep the selection visible.
fn scroll_alloc_table(mem: &mut MemoryState, direction: ScrollDir, steps: usize) {
    let row_count = alloc_row_count(mem);
    if row_count == 0 {
        return;
    }

    let current_row = mem.alloc_table_selected_row.unwrap_or(0);

    let new_row = match direction {
        ScrollDir::Up => current_row.saturating_sub(steps),
        ScrollDir::Down => (current_row + steps).min(row_count.saturating_sub(1)),
    };

    mem.alloc_table_selected_row = Some(new_row);

    let visible_height = {
        let h = mem.alloc_table_visible_height.get();
        if h == 0 {
            DEFAULT_MEM_PAGE_SIZE
        } else {
            h
        }
    };

    if new_row >= mem.alloc_table_scroll_offset + visible_height {
        mem.alloc_table_scroll_offset = new_row.saturating_sub(visible_height - 1);
    }
    if new_row < mem.alloc_table_scroll_offset {
        mem.alloc_table_scroll_offset = new_row;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::ScrollDir;
    use super::*;
    use crate::session::{AllocationSortColumn, MemorySection};
    use crate::state::{AppState, DevToolsPanel, UiMode};
    use fdemon_core::performance::AllocationProfile;

    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    fn make_state_in_memory_panel() -> AppState {
        let mut state = AppState::new();
        let device = test_device();
        state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Memory;
        state
    }

    fn make_allocation_profile_with_n(n: usize) -> AllocationProfile {
        use fdemon_core::performance::ClassHeapStats;
        AllocationProfile {
            members: (0..n)
                .map(|i| ClassHeapStats {
                    class_name: format!("Class{i}"),
                    library_uri: None,
                    new_space_instances: (i + 1) as u64,
                    new_space_size: ((i + 1) * 1024) as u64,
                    old_space_instances: 0,
                    old_space_size: 0,
                })
                .collect(),
            timestamp: chrono::Local::now(),
        }
    }

    #[test]
    fn handle_toggle_allocation_sort_cycles_columns() {
        let mut state = make_state_in_memory_panel();
        // Initial: BySize (default)
        handle_toggle_allocation_sort(&mut state);
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .allocation_sort,
            AllocationSortColumn::ByInstances,
            "First toggle should switch to ByInstances"
        );
        handle_toggle_allocation_sort(&mut state);
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .allocation_sort,
            AllocationSortColumn::BySize,
            "Second toggle should switch back to BySize"
        );
    }

    #[test]
    fn handle_mem_select_alloc_row_sets_focus() {
        let mut state = make_state_in_memory_panel();

        handle_mem_select_alloc_row(&mut state, Some(3));
        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.memory.alloc_table_selected_row,
            Some(3),
            "Should set selected row to 3"
        );
        assert_eq!(
            handle.session.memory.focused_section,
            MemorySection::AllocationList,
            "Should focus AllocationList when row is selected"
        );

        handle_mem_select_alloc_row(&mut state, None);
        let handle = state.session_manager.selected().unwrap();
        assert!(
            handle.session.memory.alloc_table_selected_row.is_none(),
            "Should clear selected row"
        );
        // focused_section does NOT revert — that would be jarring during repeated selections.
        assert_eq!(
            handle.session.memory.focused_section,
            MemorySection::AllocationList,
            "focused_section should remain AllocationList after deselect"
        );
    }

    #[test]
    fn handle_mem_focus_section_sets_section() {
        let mut state = make_state_in_memory_panel();
        handle_mem_focus_section(&mut state, MemorySection::AllocationList);
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .focused_section,
            MemorySection::AllocationList
        );

        handle_mem_focus_section(&mut state, MemorySection::Chart);
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .focused_section,
            MemorySection::Chart
        );
    }

    #[test]
    fn handle_mem_scroll_chart_scrolls_back() {
        let mut state = make_state_in_memory_panel();
        // Add some samples
        if let Some(handle) = state.session_manager.selected_mut() {
            for i in 0..50 {
                handle
                    .session
                    .memory
                    .memory_samples
                    .push(fdemon_core::performance::MemorySample {
                        dart_heap: i * 1000,
                        dart_native: 0,
                        raster_cache: 0,
                        allocated: 0,
                        rss: 0,
                        timestamp: chrono::Local::now(),
                    });
            }
            handle.session.memory.focused_section = MemorySection::Chart;
        }

        handle_mem_scroll(&mut state, ScrollDir::Up);
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .memory_chart_scroll_offset,
            1,
            "Scrolling up should increase offset by 1"
        );
    }

    #[test]
    fn handle_mem_jump_to_start_resets_scroll() {
        let mut state = make_state_in_memory_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.memory.memory_chart_scroll_offset = 10;
            handle.session.memory.focused_section = MemorySection::Chart;
        }
        handle_mem_jump_to_start(&mut state);
        // With empty samples the offset goes to 0 (0.saturating_sub(1) = 0)
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .memory_chart_scroll_offset,
            0
        );
    }

    #[test]
    fn handle_mem_jump_to_end_resets_chart_offset() {
        let mut state = make_state_in_memory_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.memory.memory_chart_scroll_offset = 10;
            handle.session.memory.focused_section = MemorySection::Chart;
        }
        handle_mem_jump_to_end(&mut state);
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .memory_chart_scroll_offset,
            0,
            "Jump to end should reset scroll offset to 0 (live edge)"
        );
    }

    #[test]
    fn handle_allocation_profile_received_stores_profile() {
        let mut state = make_state_in_memory_panel();
        let session_id = state.session_manager.selected_id().unwrap();
        let profile = make_allocation_profile_with_n(5);
        handle_allocation_profile_received(&mut state, session_id, profile);
        assert!(state
            .session_manager
            .selected()
            .unwrap()
            .session
            .memory
            .allocation_profile
            .is_some());
    }

    #[test]
    fn handle_memory_sample_received_pushes_sample() {
        let mut state = make_state_in_memory_panel();
        let session_id = state.session_manager.selected_id().unwrap();
        let sample = fdemon_core::performance::MemorySample {
            dart_heap: 1_000_000,
            dart_native: 500_000,
            raster_cache: 200_000,
            allocated: 5_000_000,
            rss: 20_000_000,
            timestamp: chrono::Local::now(),
        };
        handle_memory_sample_received(&mut state, session_id, sample);
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .memory_samples
                .len(),
            1
        );
        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .memory
                .monitoring_active
        );
    }
}
