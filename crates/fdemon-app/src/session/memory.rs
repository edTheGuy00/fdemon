//! Memory monitoring state — heap usage, GC events, allocation profile.
//!
//! Holds rolling ring-buffer history for memory snapshots, GC events, and
//! rich memory samples, plus the latest allocation profile snapshot and the
//! per-panel sort/selection state.

use std::cell::Cell;

use fdemon_core::performance::{AllocationProfile, GcEvent, MemorySample, MemoryUsage, RingBuffer};

/// Default number of memory snapshots to keep (at 2s interval = 2 minutes).
pub(crate) const DEFAULT_MEMORY_HISTORY_SIZE: usize = 60;

/// Default number of major GC events to keep.
///
/// Only major GC events (MarkSweep, MarkCompact) are stored — Scavenge events
/// are filtered out in the handler. Major GCs are rare, so 50 slots provides
/// ample history without wasting memory.
pub(crate) const DEFAULT_GC_HISTORY_SIZE: usize = 50;

/// Memory sample buffer size: 120 samples at 500ms polling = 60 seconds of history.
pub(crate) const DEFAULT_MEMORY_SAMPLE_SIZE: usize = 120;

/// Column by which the class allocation table is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllocationSortColumn {
    /// Sort by total allocated bytes (descending).
    #[default]
    BySize,
    /// Sort by total instance count (descending).
    ByInstances,
}

/// Active section within the Memory DevTools panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemorySection {
    /// Memory usage time-series chart (default section on open).
    #[default]
    Chart,
    /// Class allocation table (from `getAllocationProfile`).
    AllocationList,
}

impl MemorySection {
    /// Return the next section in Tab order (wraps around).
    pub fn next(self) -> Self {
        match self {
            MemorySection::Chart => MemorySection::AllocationList,
            MemorySection::AllocationList => MemorySection::Chart,
        }
    }

    /// Return the previous section in Tab order (wraps around).
    pub fn prev(self) -> Self {
        self.next() // 2-state cycle: next == prev
    }
}

/// Per-session memory monitoring state.
///
/// Holds rolling ring-buffer history for memory snapshots, GC events, and
/// rich memory samples, plus the latest allocation profile snapshot and the
/// per-panel sort/selection state.
#[derive(Debug, Clone)]
pub struct MemoryState {
    /// Rolling history of memory snapshots.
    pub memory_history: RingBuffer<MemoryUsage>,
    /// Rolling history of GC events.
    pub gc_history: RingBuffer<GcEvent>,
    /// Rich memory samples for time-series chart (populated by VM service polling).
    pub memory_samples: RingBuffer<MemorySample>,
    /// Latest allocation profile snapshot from `getAllocationProfile`.
    pub allocation_profile: Option<AllocationProfile>,
    /// Column by which the class allocation table is sorted.
    pub allocation_sort: AllocationSortColumn,
    /// Whether memory monitoring is active.
    pub monitoring_active: bool,

    /// Which sub-section of the Memory panel currently has keyboard focus.
    pub focused_section: MemorySection,
    /// How many samples the memory chart has been scrolled back from the live edge.
    pub memory_chart_scroll_offset: usize,
    /// Row index of the selected row in the allocation table, if any.
    pub alloc_table_selected_row: Option<usize>,
    /// Scroll offset for the allocation table (number of rows scrolled past the top).
    pub alloc_table_scroll_offset: usize,

    /// Render-hint: visible width (in columns) of the memory chart from the last rendered frame.
    ///
    /// Defaults to `0`, signalling "not yet rendered — use fallback".
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
    pub memory_chart_visible_width: Cell<usize>,

    /// Render-hint: visible height (in rows) of the allocation table from the last rendered frame.
    ///
    /// Defaults to `0`, signalling "not yet rendered — use fallback".
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
    pub alloc_table_visible_height: Cell<usize>,
}

impl Default for MemoryState {
    fn default() -> Self {
        Self {
            memory_history: RingBuffer::new(DEFAULT_MEMORY_HISTORY_SIZE),
            gc_history: RingBuffer::new(DEFAULT_GC_HISTORY_SIZE),
            memory_samples: RingBuffer::new(DEFAULT_MEMORY_SAMPLE_SIZE),
            allocation_profile: None,
            allocation_sort: AllocationSortColumn::default(),
            monitoring_active: false,
            focused_section: MemorySection::default(),
            memory_chart_scroll_offset: 0,
            alloc_table_selected_row: None,
            alloc_table_scroll_offset: 0,
            memory_chart_visible_width: Cell::new(0),
            alloc_table_visible_height: Cell::new(0),
        }
    }
}

impl MemoryState {
    /// Create a `MemoryState` with a configurable memory history size.
    ///
    /// The `memory_history_size` parameter controls how many memory snapshots to
    /// retain (ring buffer capacity). At the default 2-second poll interval,
    /// `60` snapshots covers 2 minutes of history.
    pub fn with_history_size(memory_history_size: usize) -> Self {
        Self {
            memory_history: RingBuffer::new(memory_history_size),
            ..Self::default()
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_state_default_uses_named_constants() {
        let mem = MemoryState::default();
        assert_eq!(mem.memory_history.capacity(), DEFAULT_MEMORY_HISTORY_SIZE);
        assert_eq!(mem.gc_history.capacity(), DEFAULT_GC_HISTORY_SIZE);
        assert_eq!(mem.memory_samples.capacity(), DEFAULT_MEMORY_SAMPLE_SIZE);
        assert!(mem.allocation_profile.is_none());
        assert_eq!(mem.allocation_sort, AllocationSortColumn::BySize);
        assert!(!mem.monitoring_active);
        assert_eq!(mem.focused_section, MemorySection::Chart);
    }

    #[test]
    fn memory_section_next_cycles() {
        assert_eq!(MemorySection::Chart.next(), MemorySection::AllocationList);
        assert_eq!(MemorySection::AllocationList.next(), MemorySection::Chart);
    }

    #[test]
    fn memory_section_prev_cycles_same_as_next() {
        // 2-state cycle: prev == next
        assert_eq!(MemorySection::Chart.prev(), MemorySection::AllocationList);
        assert_eq!(MemorySection::AllocationList.prev(), MemorySection::Chart);
    }

    #[test]
    fn with_history_size_overrides_default() {
        let mem = MemoryState::with_history_size(120);
        assert_eq!(mem.memory_history.capacity(), 120);
        // Other buffers use defaults.
        assert_eq!(mem.gc_history.capacity(), DEFAULT_GC_HISTORY_SIZE);
        assert_eq!(mem.memory_samples.capacity(), DEFAULT_MEMORY_SAMPLE_SIZE);
    }

    #[test]
    fn memory_state_default_scroll_and_selection_are_zero() {
        let mem = MemoryState::default();
        assert_eq!(mem.memory_chart_scroll_offset, 0);
        assert_eq!(mem.alloc_table_scroll_offset, 0);
        assert!(mem.alloc_table_selected_row.is_none());
        assert_eq!(mem.memory_chart_visible_width.get(), 0);
        assert_eq!(mem.alloc_table_visible_height.get(), 0);
    }
}
