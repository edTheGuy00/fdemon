//! Tests for the [`MemoryPanel`] widget and [`BrailleCanvas`].

use super::*;
use fdemon_app::session::memory::{AllocationSortColumn, MemorySection, MemoryState};
use fdemon_core::performance::{AllocationProfile, ClassHeapStats, GcEvent, MemoryUsage};

// ── Test helpers ──────────────────────────────────────────────────────────────

/// Build a `MemorySample` where `dart_heap` encodes the sample index.
fn sample_at(i: u64) -> MemorySample {
    MemorySample {
        dart_heap: i,
        dart_native: 0,
        raster_cache: 0,
        allocated: 0,
        rss: 0,
        timestamp: chrono::Local::now(),
    }
}

fn make_sample(heap: u64, native: u64, raster: u64, allocated: u64, rss: u64) -> MemorySample {
    MemorySample {
        dart_heap: heap,
        dart_native: native,
        raster_cache: raster,
        allocated,
        rss,
        timestamp: chrono::Local::now(),
    }
}

fn make_memory_usage(usage: u64, capacity: u64) -> MemoryUsage {
    MemoryUsage {
        heap_usage: usage,
        heap_capacity: capacity,
        external_usage: 0,
        timestamp: chrono::Local::now(),
    }
}

/// Build a `MemoryState` with optional samples, history, and allocation profile.
fn make_memory_state() -> MemoryState {
    MemoryState::default()
}

fn make_memory_state_with_samples(n: u64) -> MemoryState {
    let mut state = MemoryState::default();
    for i in 0..n {
        state.memory_samples.push(make_sample(
            (i + 1) * 1_000_000,
            200_000,
            50_000,
            10_000_000,
            20_000_000,
        ));
    }
    state
}

fn make_memory_state_with_history(n: usize) -> MemoryState {
    let mut state = MemoryState::default();
    for i in 0..n {
        state
            .memory_history
            .push(make_memory_usage((i as u64 + 1) * 5_000_000, 128_000_000));
    }
    state
}

/// Build a profile with `n` classes named "Class0", "Class1", etc.
fn mock_profile_with_n_classes(n: usize) -> AllocationProfile {
    let members = (0..n)
        .map(|i| ClassHeapStats {
            class_name: format!("Class{i}"),
            library_uri: None,
            new_space_instances: (i as u64) + 1,
            new_space_size: ((n - i) as u64) * 1_000,
            old_space_instances: 0,
            old_space_size: 0,
        })
        .collect();
    AllocationProfile {
        members,
        timestamp: chrono::Local::now(),
    }
}

/// Build a two-class profile where ClassA has larger size, ClassB has more instances.
fn make_two_class_profile() -> AllocationProfile {
    AllocationProfile {
        members: vec![
            ClassHeapStats {
                class_name: "ClassA".to_string(),
                library_uri: None,
                new_space_instances: 10,
                new_space_size: 1_000_000,
                old_space_instances: 5,
                old_space_size: 500_000,
            },
            ClassHeapStats {
                class_name: "ClassB".to_string(),
                library_uri: None,
                new_space_instances: 5_000,
                new_space_size: 10_000,
                old_space_instances: 2_000,
                old_space_size: 5_000,
            },
        ],
        timestamp: chrono::Local::now(),
    }
}

fn buffer_content(buf: &Buffer, area: Rect) -> String {
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (area.x + x, area.y + y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect()
}

// ── visible_memory_window tests ──────────────────────────────────────────────

#[test]
fn memory_chart_window_at_offset() {
    let samples: Vec<MemorySample> = (0..120).map(sample_at).collect();
    let window = visible_memory_window(&samples, 60, 30);
    assert_eq!(window.len(), 60);
    assert_eq!(window.first().unwrap().dart_heap, 30);
    assert_eq!(window.last().unwrap().dart_heap, 89);
}

#[test]
fn memory_chart_window_at_live_edge() {
    let samples: Vec<MemorySample> = (0..120).map(sample_at).collect();
    let window = visible_memory_window(&samples, 60, 0);
    assert_eq!(window.last().unwrap().dart_heap, 119);
    assert_eq!(window.len(), 60);
}

#[test]
fn memory_chart_window_fewer_samples_than_width() {
    let samples: Vec<MemorySample> = (0..10).map(sample_at).collect();
    let window = visible_memory_window(&samples, 60, 0);
    assert_eq!(window.len(), 10);
}

#[test]
fn memory_chart_window_offset_beyond_len_returns_empty() {
    let samples: Vec<MemorySample> = (0..10).map(sample_at).collect();
    let window = visible_memory_window(&samples, 60, 20);
    assert_eq!(window.len(), 0);
}

#[test]
fn memory_chart_window_exact_fit() {
    let samples: Vec<MemorySample> = (0..60).map(sample_at).collect();
    let window = visible_memory_window(&samples, 60, 0);
    assert_eq!(window.len(), 60);
    assert_eq!(window.first().unwrap().dart_heap, 0);
    assert_eq!(window.last().unwrap().dart_heap, 59);
}

// ── BrailleCanvas tests ──────────────────────────────────────────────────────

#[test]
fn test_braille_canvas_single_dot() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set(0, 0);
    assert_eq!(canvas.cells[0][0], 0x01);
    let expected_char = char::from_u32(0x2800 + 0x01).unwrap();
    assert_eq!(expected_char, '\u{2801}');
}

#[test]
fn test_braille_canvas_all_dots_in_cell() {
    let mut canvas = BrailleCanvas::new(1, 1);
    for y in 0..4 {
        for x in 0..2 {
            canvas.set(x, y);
        }
    }
    assert_eq!(canvas.cells[0][0], 0xFF);
}

#[test]
fn test_braille_canvas_out_of_bounds_ignored() {
    let mut canvas = BrailleCanvas::new(2, 2);
    canvas.set(100, 100);
    canvas.set(4, 0);
    canvas.set(0, 8);
    for row in &canvas.cells {
        for &cell in row {
            assert_eq!(cell, 0);
        }
    }
}

#[test]
fn test_braille_canvas_multi_cell() {
    let mut canvas = BrailleCanvas::new(3, 2);
    canvas.set(0, 0);
    canvas.set(5, 7);
    assert_eq!(canvas.cells[0][0], 0x01);
    assert_eq!(canvas.cells[1][2], 0x80);
}

#[test]
fn test_braille_canvas_second_column_dots() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set(1, 0);
    assert_eq!(canvas.cells[0][0], 0x08);
}

#[test]
fn test_braille_canvas_renders_to_buffer() {
    let mut canvas = BrailleCanvas::new(2, 2);
    canvas.set(0, 0);
    canvas.set(2, 4);
    let area = Rect::new(0, 0, 2, 2);
    let mut buf = Buffer::empty(area);
    canvas.render_to_buffer(&mut buf, area, Color::Cyan);
    let cell_00 = buf.cell((0u16, 0u16)).unwrap();
    assert_eq!(cell_00.symbol(), "\u{2801}");
    let cell_11 = buf.cell((1u16, 1u16)).unwrap();
    assert_eq!(cell_11.symbol(), "\u{2801}");
}

#[test]
fn test_braille_canvas_empty_cells_not_rendered() {
    let canvas = BrailleCanvas::new(2, 2);
    let area = Rect::new(0, 0, 2, 2);
    let mut buf = Buffer::empty(area);
    canvas.render_to_buffer(&mut buf, area, Color::Cyan);
    for y in 0..2u16 {
        for x in 0..2u16 {
            let cell = buf.cell((x, y)).unwrap();
            assert!(!cell.symbol().contains('\u{2801}'));
        }
    }
}

// ── MemoryPanel widget tests ─────────────────────────────────────────────────

#[test]
fn test_renders_empty_state_without_panic() {
    let mem = make_memory_state();
    let widget = MemoryPanel::new(&mem, true);
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_renders_with_memory_usage_fallback() {
    let mem = make_memory_state_with_history(10);
    let widget = MemoryPanel::new(&mem, true);
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_renders_single_sample_without_panic() {
    let mem = make_memory_state_with_samples(1);
    let widget = MemoryPanel::new(&mem, false);
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_renders_full_buffer_without_panic() {
    let mem = make_memory_state_with_samples(120);
    let widget = MemoryPanel::new(&mem, true);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_compact_mode_small_height() {
    let mem = make_memory_state();
    let widget = MemoryPanel::new(&mem, false);
    let area = Rect::new(0, 0, 80, 5);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_very_small_area_no_panic() {
    let mem = make_memory_state();
    let widget = MemoryPanel::new(&mem, false);
    let area = Rect::new(0, 0, 10, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_zero_area_no_panic() {
    let mem = make_memory_state();
    let widget = MemoryPanel::new(&mem, false);
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_chart_only_mode_no_table() {
    let mem = make_memory_state_with_samples(1);
    let widget = MemoryPanel::new(&mem, false);
    let area = Rect::new(0, 0, 80, 7);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_allocation_table_visible_at_threshold() {
    let mem = make_memory_state_with_samples(1);
    let widget = MemoryPanel::new(&mem, false);
    let area = Rect::new(0, 0, 80, 8);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let content = buffer_content(&buf, area);
    assert!(
        content.contains("loading")
            || content.contains("Class")
            || content.contains("Instances")
            || content.contains("Waiting"),
        "Allocation table should be visible at height 8 (threshold); content: {content:?}"
    );
}

// ── Chart rendering helpers tests ────────────────────────────────────────────

#[test]
fn test_legend_omits_raster_when_zero() {
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    samples.push(make_sample(50_000_000, 10_000_000, 0, 128_000_000, 0));

    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    render_legend(&samples, &memory_history, area, &mut buf);

    let content: String = (0..80u16)
        .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(content.contains("Heap"), "Legend should show Heap");
    assert!(
        !content.contains("Raster"),
        "Raster should not appear when all raster_cache=0"
    );
}

#[test]
fn test_compact_summary_with_samples() {
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    samples.push(make_sample(
        50_000_000,
        10_000_000,
        0,
        128_000_000,
        200_000_000,
    ));

    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    render_compact_summary(&samples, &history, area, &mut buf);

    let content: String = (0..80u16)
        .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(content.contains("Heap") || content.contains("MB"));
}

#[test]
fn test_compact_summary_no_data() {
    let samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let history: RingBuffer<MemoryUsage> = RingBuffer::new(10);

    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    render_compact_summary(&samples, &history, area, &mut buf);

    let content: String = (0..80u16)
        .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(content.contains("No memory data"));
}

#[test]
fn test_gc_event_marker_renders_without_panic() {
    let mut state = MemoryState::default();
    let now = chrono::Local::now();
    let old_ts = now - chrono::Duration::seconds(30);

    let mut s1 = make_sample(50_000_000, 0, 0, 100_000_000, 0);
    s1.timestamp = old_ts;
    state.memory_samples.push(s1);

    let mut s2 = make_sample(60_000_000, 0, 0, 100_000_000, 0);
    s2.timestamp = now;
    state.memory_samples.push(s2);

    state.gc_history.push(GcEvent {
        gc_type: "MarkSweep".to_string(),
        reason: None,
        isolate_id: None,
        timestamp: old_ts + chrono::Duration::seconds(15),
    });

    let widget = MemoryPanel::new(&state, true);
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

// ── Allocation table function tests ─────────────────────────────────────────

#[test]
fn test_allocation_table_shows_top_classes() {
    let profile = AllocationProfile {
        members: vec![
            ClassHeapStats {
                class_name: "dart:core/String".to_string(),
                library_uri: None,
                new_space_instances: 1000,
                new_space_size: 500_000,
                old_space_instances: 500,
                old_space_size: 300_000,
            },
            ClassHeapStats {
                class_name: "dart:core/_List".to_string(),
                library_uri: None,
                new_space_instances: 200,
                new_space_size: 100_000,
                old_space_instances: 100,
                old_space_size: 50_000,
            },
        ],
        timestamp: chrono::Local::now(),
    };

    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);

    let content = buffer_content(&buf, area);
    assert!(content.contains("String"), "Should display String class");
    assert!(content.contains("_List"), "Should display _List class");
}

#[test]
fn test_allocation_table_none_profile() {
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(None, AllocationSortColumn::BySize, area, &mut buf);

    let content = buffer_content(&buf, area);
    assert!(
        content.contains("Waiting"),
        "Should show waiting message when profile is None"
    );
}

// ── UTF-8 truncation tests ────────────────────────────────────────────────────

#[test]
fn test_class_name_truncation_with_cjk() {
    let long_cjk = "这是一个非常长的类名称用于测试截断功能是否正确工作还有更多内容确保超三十";
    assert!(long_cjk.chars().count() > 30);
    let profile = AllocationProfile {
        members: vec![ClassHeapStats {
            class_name: long_cjk.to_string(),
            library_uri: None,
            new_space_instances: 100,
            new_space_size: 50_000,
            old_space_instances: 50,
            old_space_size: 25_000,
        }],
        timestamp: chrono::Local::now(),
    };
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
}

#[test]
fn test_class_name_truncation_with_emoji() {
    let emoji_name = "MyClass🎉🎊🎈PaddingToMakeItLongEnoughToTruncate";
    assert!(emoji_name.chars().count() > 30);
    let profile = AllocationProfile {
        members: vec![ClassHeapStats {
            class_name: emoji_name.to_string(),
            library_uri: None,
            new_space_instances: 10,
            new_space_size: 1_000,
            old_space_instances: 5,
            old_space_size: 500,
        }],
        timestamp: chrono::Local::now(),
    };
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
}

#[test]
fn test_class_name_truncation_result_ends_with_ellipsis() {
    let long_ascii = "AVeryLongClassNameThatDefinitelyExceedsThirtyChars";
    assert!(long_ascii.chars().count() > 30);
    let profile = AllocationProfile {
        members: vec![ClassHeapStats {
            class_name: long_ascii.to_string(),
            library_uri: None,
            new_space_instances: 1,
            new_space_size: 100,
            old_space_instances: 0,
            old_space_size: 0,
        }],
        timestamp: chrono::Local::now(),
    };
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
    let content = buffer_content(&buf, area);
    assert!(
        content.contains("..."),
        "Truncated name should end with '...'"
    );
    assert!(
        content.contains(&long_ascii[..27]),
        "Truncated name should start with first 27 chars"
    );
}

#[test]
fn test_class_name_no_truncation_for_short_name() {
    let short_name = "dart:core/String";
    assert!(short_name.chars().count() <= 30);
    let profile = AllocationProfile {
        members: vec![ClassHeapStats {
            class_name: short_name.to_string(),
            library_uri: None,
            new_space_instances: 500,
            new_space_size: 200_000,
            old_space_instances: 200,
            old_space_size: 100_000,
        }],
        timestamp: chrono::Local::now(),
    };
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
    let content = buffer_content(&buf, area);
    assert!(
        content.contains("dart:core/String"),
        "Short name should be rendered in full"
    );
}

// ── Allocation table sorting tests ───────────────────────────────────────────

#[test]
fn test_allocation_table_sort_by_size_renders_size_indicator() {
    let profile = make_two_class_profile();
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
    let content = buffer_content(&buf, area);
    assert!(
        content.contains('\u{25bc}'),
        "BySize sort should show ▼ indicator in header"
    );
}

#[test]
fn test_allocation_table_sort_by_instances_renders_instances_indicator() {
    let profile = make_two_class_profile();
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(
        Some(&profile),
        AllocationSortColumn::ByInstances,
        area,
        &mut buf,
    );
    let content = buffer_content(&buf, area);
    assert!(
        content.contains('\u{25bc}'),
        "ByInstances sort should show ▼ indicator in header"
    );
}

#[test]
fn test_allocation_table_by_size_shows_class_a_first() {
    let profile = make_two_class_profile();
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
    let content = buffer_content(&buf, area);
    let pos_a = content.find("ClassA");
    let pos_b = content.find("ClassB");
    assert!(
        pos_a.is_some() && pos_b.is_some(),
        "Both classes should appear"
    );
    assert!(
        pos_a.unwrap() < pos_b.unwrap(),
        "BySize: ClassA (larger bytes) should appear before ClassB"
    );
}

#[test]
fn test_allocation_table_by_instances_shows_class_b_first() {
    let profile = make_two_class_profile();
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(
        Some(&profile),
        AllocationSortColumn::ByInstances,
        area,
        &mut buf,
    );
    let content = buffer_content(&buf, area);
    let pos_a = content.find("ClassA");
    let pos_b = content.find("ClassB");
    assert!(
        pos_a.is_some() && pos_b.is_some(),
        "Both classes should appear"
    );
    assert!(
        pos_b.unwrap() < pos_a.unwrap(),
        "ByInstances: ClassB (more instances) should appear before ClassA"
    );
}

// ── AllocationTable struct tests ─────────────────────────────────────────────

#[test]
fn alloc_table_visible_height_cell_written_each_frame() {
    let profile = mock_profile_with_n_classes(50);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let cell = std::cell::Cell::new(0usize);

    let table = AllocationTable {
        profile: &profile,
        sort_column: AllocationSortColumn::BySize,
        scroll_offset: 0,
        selected_row: None,
        focused: false,
        visible_height_cell: &cell,
    };
    table.render(area, &mut buf, None);

    assert_eq!(
        cell.get(),
        10,
        "visible_height should be area.height({}) - TABLE_HEADER_ROWS({})",
        area.height,
        table::TABLE_HEADER_ROWS
    );
}

#[test]
fn alloc_table_renders_windowed_slice_at_offset_zero() {
    let profile = mock_profile_with_n_classes(5);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let cell = std::cell::Cell::new(0usize);

    let table = AllocationTable {
        profile: &profile,
        sort_column: AllocationSortColumn::BySize,
        scroll_offset: 0,
        selected_row: None,
        focused: false,
        visible_height_cell: &cell,
    };
    table.render(area, &mut buf, None);

    let content = buffer_content(&buf, area);
    assert!(
        content.contains("Class0"),
        "First visible row at offset 0 should be Class0 (largest size)"
    );
}

#[test]
fn alloc_table_renders_windowed_slice_at_positive_offset() {
    let profile = mock_profile_with_n_classes(50);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let cell = std::cell::Cell::new(0usize);

    let table = AllocationTable {
        profile: &profile,
        sort_column: AllocationSortColumn::BySize,
        scroll_offset: 20,
        selected_row: None,
        focused: false,
        visible_height_cell: &cell,
    };
    table.render(area, &mut buf, None);

    let content = buffer_content(&buf, area);
    assert!(
        content.contains("Class20"),
        "Class20 should be the first visible row at scroll_offset=20"
    );
    assert!(
        !content.contains("Class0"),
        "Class0 should be scrolled out of view"
    );
}

#[test]
fn alloc_table_selected_row_highlighted_when_visible() {
    let profile = mock_profile_with_n_classes(50);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let cell = std::cell::Cell::new(0usize);

    let table = AllocationTable {
        profile: &profile,
        sort_column: AllocationSortColumn::BySize,
        scroll_offset: 20,
        selected_row: Some(25),
        focused: true,
        visible_height_cell: &cell,
    };
    table.render(area, &mut buf, None);

    let selected_y = table::TABLE_HEADER_ROWS as u16 + (25 - 20) as u16;
    let has_highlight = (0..area.width)
        .filter_map(|x| buf.cell((area.x + x, area.y + selected_y)))
        .any(|c| c.bg != ratatui::style::Color::Reset);
    assert!(
        has_highlight,
        "Selected row at y={selected_y} should have a highlighted background"
    );
}

#[test]
fn alloc_table_clicking_row_emits_mem_select_alloc_row() {
    use fdemon_app::{Message, MouseButton, MouseRegions};

    let profile = mock_profile_with_n_classes(50);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let cell = std::cell::Cell::new(0usize);

    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);

        let table = AllocationTable {
            profile: &profile,
            sort_column: AllocationSortColumn::BySize,
            scroll_offset: 20,
            selected_row: None,
            focused: false,
            visible_height_cell: &cell,
        };
        table.render(area, &mut buf, Some(&mut ctx));
    }

    let click_y = area.y + table::TABLE_HEADER_ROWS as u16;
    let hit = regions.hit_test(area.x, click_y, MouseButton::Left);
    assert!(
        hit.is_some(),
        "Should have a click region at row 0 of data area"
    );

    let msg = hit
        .and_then(|e| e.on_left.as_ref())
        .and_then(|a| a.as_emit());
    assert!(
        matches!(msg, Some(Message::MemSelectAllocRow { index: Some(20) })),
        "Clicking the first visible row at scroll_offset=20 should emit global index 20; got {msg:?}"
    );
}

#[test]
fn alloc_table_empty_space_emits_mem_focus_section() {
    use fdemon_app::{Message, MouseButton, MouseRegions};

    let profile = mock_profile_with_n_classes(3);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let cell = std::cell::Cell::new(0usize);

    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);

        let table = AllocationTable {
            profile: &profile,
            sort_column: AllocationSortColumn::BySize,
            scroll_offset: 0,
            selected_row: None,
            focused: false,
            visible_height_cell: &cell,
        };
        table.render(area, &mut buf, Some(&mut ctx));
    }

    let empty_y = area.y + table::TABLE_HEADER_ROWS as u16 + 3;
    let hit = regions.hit_test(area.x, empty_y, MouseButton::Left);
    let msg = hit
        .and_then(|e| e.on_left.as_ref())
        .and_then(|a| a.as_emit());
    assert!(
        matches!(
            msg,
            Some(Message::MemFocusSection(MemorySection::AllocationList))
        ),
        "Clicking empty space below rows should emit MemFocusSection(AllocationList); got {msg:?}"
    );
}

#[test]
fn alloc_table_no_focus_region_when_rows_fill_area() {
    use fdemon_app::{Message, MouseButton, MouseRegions};

    let profile = mock_profile_with_n_classes(50);
    let area = Rect::new(0, 0, 80, 12);
    let mut buf = Buffer::empty(area);
    let cell = std::cell::Cell::new(0usize);

    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = crate::render::MouseCtx::new(builder);

        let table = AllocationTable {
            profile: &profile,
            sort_column: AllocationSortColumn::BySize,
            scroll_offset: 0,
            selected_row: None,
            focused: false,
            visible_height_cell: &cell,
        };
        table.render(area, &mut buf, Some(&mut ctx));
    }

    let beyond_last_y = area.y + area.height;
    let oob_hit = regions.hit_test(area.x, beyond_last_y, MouseButton::Left);
    assert!(
        oob_hit.is_none(),
        "No click region should exist beyond the table area"
    );

    let last_row_y = area.y + table::TABLE_HEADER_ROWS as u16 + 9;
    let last_hit = regions.hit_test(area.x, last_row_y, MouseButton::Left);
    let last_msg = last_hit
        .and_then(|e| e.on_left.as_ref())
        .and_then(|a| a.as_emit())
        .cloned();
    assert!(
        matches!(last_msg, Some(Message::MemSelectAllocRow { .. })),
        "Last visible row should emit MemSelectAllocRow"
    );
}

// ── Regression test: allocation table full height at 20 rows ─────────────────

#[test]
fn test_memory_panel_allocation_table_full_height_at_20_rows() {
    // Build a MemoryState with 30 distinct classes in the allocation profile.
    let mem = MemoryState {
        allocation_profile: Some(mock_profile_with_n_classes(30)),
        allocation_sort: AllocationSortColumn::BySize,
        ..MemoryState::default()
    };

    // Render into a 200×20 terminal (mimicking the bug-report scenario).
    let widget = MemoryPanel::new(&mem, true);
    let mut buf = Buffer::empty(Rect::new(0, 0, 200, 20));
    widget.render(Rect::new(0, 0, 200, 20), &mut buf);

    // Count rows in the table area that contain class names (start with 'Class').
    fn count_rows_with_class(buf: &Buffer, width: u16, height: u16) -> usize {
        (0..height)
            .filter(|&y| {
                let row: String = (0..width)
                    .filter_map(|x| buf.cell((x, y)).map(|c| c.symbol().to_string()))
                    .collect();
                row.contains("Class")
            })
            .count()
    }

    let count = count_rows_with_class(&buf, 200, 20);
    assert!(
        count >= 6,
        "expected ≥ 6 visible alloc-table rows in 20-row terminal (full panel), got {count}"
    );
}

// ── Memory panel with allocation profile visible ──────────────────────────────

#[test]
fn test_memory_panel_renders_allocation_profile() {
    let mem = MemoryState {
        allocation_profile: Some(AllocationProfile {
            members: vec![ClassHeapStats {
                class_name: "dart:core/String".to_string(),
                library_uri: None,
                new_space_instances: 1000,
                new_space_size: 500_000,
                old_space_instances: 500,
                old_space_size: 300_000,
            }],
            timestamp: chrono::Local::now(),
        }),
        monitoring_active: true,
        ..MemoryState::default()
    };

    let widget = MemoryPanel::new(&mem, true);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    let content = buffer_content(&buf, area);
    assert!(
        content.contains("String"),
        "Memory panel should display class names from allocation profile; content: {content:?}"
    );
}

// ── Memory panel allocation table tests migrated from performance/tests.rs ────

#[test]
fn test_memory_panel_no_stats_section() {
    let mem = MemoryState {
        monitoring_active: true,
        ..MemoryState::default()
    };
    let widget = MemoryPanel::new(&mem, true);
    let area = Rect::new(0, 0, 80, 30);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let content = buffer_content(&buf, area);
    assert!(
        !content.contains(" Stats "),
        "Stats section should not exist in Memory panel"
    );
}

#[test]
fn test_memory_panel_allocation_table_visible_on_24_row_terminal() {
    let mem = MemoryState {
        monitoring_active: true,
        ..MemoryState::default()
    };
    let widget = MemoryPanel::new(&mem, true);
    let area = Rect::new(0, 0, 80, 18);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let content = buffer_content(&buf, area);
    assert!(
        content.contains("loading")
            || content.contains("Class")
            || content.contains("Instances")
            || content.contains("Waiting"),
        "Allocation table should be visible at 18 rows; content: {content:?}"
    );
}

#[test]
fn test_memory_panel_allocation_table_visible_on_30_row_terminal() {
    let mem = MemoryState {
        monitoring_active: true,
        ..MemoryState::default()
    };
    let widget = MemoryPanel::new(&mem, true);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    let content = buffer_content(&buf, area);
    assert!(
        content.contains("loading")
            || content.contains("Class")
            || content.contains("Instances")
            || content.contains("Waiting"),
        "Allocation table should be visible at 24 rows; content: {content:?}"
    );
}
