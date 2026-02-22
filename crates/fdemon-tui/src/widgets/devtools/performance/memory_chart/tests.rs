//! Tests for the [`MemoryChart`] widget and [`BrailleCanvas`].

use super::*;
use fdemon_core::performance::{AllocationProfile, ClassHeapStats, GcEvent, MemoryUsage};

// ── BrailleCanvas tests ──────────────────────────────────────────────────

#[test]
fn test_braille_canvas_single_dot() {
    let mut canvas = BrailleCanvas::new(1, 1);
    canvas.set(0, 0); // top-left dot (dot 1 in braille standard)
                      // Dot 1 bit is 0x01 → U+2801
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
    // All 8 dots set: 0x01 | 0x08 | 0x02 | 0x10 | 0x04 | 0x20 | 0x40 | 0x80 = 0xFF
    assert_eq!(canvas.cells[0][0], 0xFF);
    // U+28FF
    let expected_char = char::from_u32(0x2800 + 0xFF).unwrap();
    assert_eq!(expected_char, '\u{28FF}');
}

#[test]
fn test_braille_canvas_out_of_bounds_ignored() {
    let mut canvas = BrailleCanvas::new(2, 2);
    // Should not panic on out-of-bounds coordinates
    canvas.set(100, 100);
    canvas.set(4, 0); // x=4 → col=2 which is out of bounds for width=2
    canvas.set(0, 8); // y=8 → row=2 which is out of bounds for height=2
                      // All cells should remain zero
    for row in &canvas.cells {
        for &cell in row {
            assert_eq!(cell, 0);
        }
    }
}

#[test]
fn test_braille_canvas_multi_cell() {
    let mut canvas = BrailleCanvas::new(3, 2);
    canvas.set(0, 0); // cell (0, 0)
    canvas.set(5, 7); // col = 5/2 = 2, row = 7/4 = 1 → cell (1, 2)
    assert_eq!(canvas.cells[0][0], 0x01); // dot 1
                                          // y%4=3, x%2=1 → BRAILLE_BIT_MAP[3][1] = 0x80
    assert_eq!(canvas.cells[1][2], 0x80);
}

#[test]
fn test_braille_canvas_second_column_dots() {
    let mut canvas = BrailleCanvas::new(1, 1);
    // x=1 → right column of the cell; y=0 → row 0
    // BRAILLE_BIT_MAP[0][1] = 0x08 (dot 4)
    canvas.set(1, 0);
    assert_eq!(canvas.cells[0][0], 0x08);
}

#[test]
fn test_braille_canvas_renders_to_buffer() {
    let mut canvas = BrailleCanvas::new(2, 2);
    canvas.set(0, 0);
    canvas.set(2, 4); // second cell column, second cell row
    let area = Rect::new(0, 0, 2, 2);
    let mut buf = Buffer::empty(area);
    canvas.render_to_buffer(&mut buf, area, Color::Cyan);

    // Cell (0,0) should have braille char U+2801
    let cell_00 = buf.cell((0u16, 0u16)).unwrap();
    assert_eq!(cell_00.symbol(), "\u{2801}");
    // Cell (1,1) should have braille char U+2801 (same dot pattern, different cell)
    let cell_11 = buf.cell((1u16, 1u16)).unwrap();
    assert_eq!(cell_11.symbol(), "\u{2801}");
}

#[test]
fn test_braille_canvas_empty_cells_not_rendered() {
    let canvas = BrailleCanvas::new(2, 2);
    let area = Rect::new(0, 0, 2, 2);
    let mut buf = Buffer::empty(area);
    canvas.render_to_buffer(&mut buf, area, Color::Cyan);
    // Empty canvas → no braille chars written, cells should remain as space
    for y in 0..2u16 {
        for x in 0..2u16 {
            let cell = buf.cell((x, y)).unwrap();
            // Empty cells render nothing (we only write non-zero bits)
            assert!(!cell.symbol().contains('\u{2801}'));
        }
    }
}

// ── Chart rendering tests ────────────────────────────────────────────────

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

#[test]
fn test_renders_empty_samples_without_panic() {
    let samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);
    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Should not panic
}

#[test]
fn test_renders_with_memory_usage_fallback() {
    let samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let mut memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    for i in 0..10 {
        memory_history.push(make_memory_usage((i + 1) * 5_000_000, 128_000_000));
    }

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Should not panic and should use fallback rendering
}

#[test]
fn test_renders_single_sample_without_panic() {
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    samples.push(make_sample(
        50_000_000,
        10_000_000,
        5_000_000,
        128_000_000,
        200_000_000,
    ));

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_renders_full_buffer_without_panic() {
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    for i in 0..120u64 {
        samples.push(make_sample(
            (i + 1) * 1_000_000,
            2_000_000,
            500_000,
            150_000_000,
            300_000_000,
        ));
    }

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_legend_omits_raster_when_zero() {
    // All raster_cache values are 0 → "Raster" should not appear in legend
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    let _gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

    samples.push(make_sample(50_000_000, 10_000_000, 0, 128_000_000, 0));

    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    render_legend(&samples, &memory_history, area, &mut buf);

    let content: String = (0..80u16)
        .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(
        !content.contains("Raster"),
        "Raster should be omitted when all zero"
    );
}

#[test]
fn test_legend_omits_rss_when_zero() {
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    let _gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

    samples.push(make_sample(
        50_000_000,
        10_000_000,
        5_000_000,
        128_000_000,
        0,
    ));

    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    render_legend(&samples, &memory_history, area, &mut buf);

    let content: String = (0..80u16)
        .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(
        !content.contains("RSS"),
        "RSS should be omitted when all zero"
    );
}

#[test]
fn test_legend_includes_raster_when_nonzero() {
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    let _gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

    samples.push(make_sample(
        50_000_000,
        10_000_000,
        5_000_000,
        128_000_000,
        0,
    ));

    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    render_legend(&samples, &memory_history, area, &mut buf);

    let content: String = (0..80u16)
        .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(
        content.contains("Raster"),
        "Raster should appear when nonzero"
    );
}

#[test]
fn test_gc_markers_positioned_correctly() {
    // With samples present and a GC event within the time range,
    // the chart should not panic and should render a GC marker.
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let mut gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    let now = chrono::Local::now();
    let old_ts = now - chrono::Duration::seconds(30);

    // Push two samples bracketing the GC event
    let mut s1 = make_sample(50_000_000, 0, 0, 100_000_000, 0);
    s1.timestamp = old_ts;
    samples.push(s1);

    let mut s2 = make_sample(60_000_000, 0, 0, 100_000_000, 0);
    s2.timestamp = now;
    samples.push(s2);

    // GC event in between
    gc_history.push(GcEvent {
        gc_type: "MarkSweep".to_string(),
        reason: None,
        isolate_id: None,
        timestamp: old_ts + chrono::Duration::seconds(15),
    });

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Should not panic; GC marker should be written somewhere
}

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

    // Collect all text from the buffer
    let content: String = (0..10u16)
        .flat_map(|y| (0..80u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect();

    assert!(content.contains("String"), "Should display String class");
    assert!(content.contains("_List"), "Should display _List class");
}

#[test]
fn test_allocation_table_none_profile() {
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_allocation_table(None, AllocationSortColumn::BySize, area, &mut buf);

    let content: String = (0..10u16)
        .flat_map(|y| (0..80u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect();

    assert!(
        content.contains("Waiting"),
        "Should show waiting message when profile is None"
    );
}

#[test]
fn test_compact_mode_small_height() {
    // area height < MIN_CHART_HEIGHT (6) → compact summary
    let samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 80, 5); // height = 5 < 6
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // Should not panic — renders compact summary
}

#[test]
fn test_very_small_area_no_panic() {
    let samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 10, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_zero_area_no_panic() {
    let samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 0, 0);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_y_axis_auto_scaling() {
    // Samples with max ~50MB: y-axis should scale accordingly (no panic)
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    for i in 1..=10u64 {
        samples.push(make_sample(i * 5_000_000, 0, 0, 60_000_000, 0));
    }

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Check y-axis area for "MB" text
    let content: String = (0..20u16)
        .flat_map(|y| (0..7u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(
        content.contains("MB") || content.contains("KB"),
        "Y-axis should show memory units"
    );
}

#[test]
fn test_chart_only_mode_no_table() {
    // Total height in [MIN_CHART_HEIGHT, MIN_CHART_HEIGHT + MIN_TABLE_HEIGHT - 1]
    // → shows chart only, no table.
    // MIN_TABLE_HEIGHT is now 2, so threshold is 6+2=8. Height 7 is chart-only.
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

    samples.push(make_sample(50_000_000, 5_000_000, 0, 100_000_000, 0));

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    // height = 7 is >= MIN_CHART_HEIGHT(6) but < MIN_CHART_HEIGHT+MIN_TABLE_HEIGHT (8)
    let area = Rect::new(0, 0, 80, 7);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // No panic, no "loading" or class table text expected
}

#[test]
fn test_allocation_table_visible_at_threshold() {
    // Height exactly at MIN_CHART_HEIGHT + MIN_TABLE_HEIGHT (= 6 + 2 = 8)
    // → show_table should be true, allocation table renders
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

    samples.push(make_sample(50_000_000, 5_000_000, 0, 100_000_000, 0));

    let widget = MemoryChart::new(
        &samples,
        &memory_history,
        &gc_history,
        None,
        AllocationSortColumn::BySize,
        false,
    );
    // height = 8 is exactly MIN_CHART_HEIGHT(6) + MIN_TABLE_HEIGHT(2)
    let area = Rect::new(0, 0, 80, 8);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Should render the allocation table header ("loading..." or "Class")
    let content: String = (0..8u16)
        .flat_map(|y| (0..80u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(
        content.contains("loading") || content.contains("Class") || content.contains("Instances"),
        "Allocation table should be visible at height 8 (threshold); content: {content:?}"
    );
}

#[test]
fn test_format_number_with_commas() {
    // format_number is re-exported from styles.rs — test the same cases
    use super::super::styles::format_number;
    assert_eq!(format_number(0), "0");
    assert_eq!(format_number(999), "999");
    assert_eq!(format_number(1_000), "1,000");
    assert_eq!(format_number(1_234_567), "1,234,567");
    assert_eq!(format_number(12_345), "12,345");
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

// ── UTF-8 truncation tests ────────────────────────────────────────────────

#[test]
fn test_class_name_truncation_with_cjk() {
    // CJK class name longer than 30 characters (each CJK char is 3 bytes, so
    // byte-indexing by 27 would panic mid-codepoint without the char-based fix)
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
    // Must not panic
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
}

#[test]
fn test_class_name_truncation_with_emoji() {
    // Emoji are 4-byte sequences — byte-indexing would panic without the fix
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
    // Must not panic
    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);
}

#[test]
fn test_class_name_truncation_result_ends_with_ellipsis() {
    // Verify that a long ASCII name gets truncated with "..."
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
    // The rendered row should contain "..." and the first 27 chars of the name
    let content: String = (0..10u16)
        .flat_map(|y| (0..80u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect();
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
    // Short name (<= 30 chars) must be rendered in full, no ellipsis
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
    let content: String = (0..10u16)
        .flat_map(|y| (0..80u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect();
    assert!(
        content.contains("dart:core/String"),
        "Short name should be rendered in full"
    );
}

// ── Allocation table sorting tests ────────────────────────────────────────

/// Build a two-class profile where `ClassA` has a larger size but fewer
/// instances than `ClassB`, so BySize and ByInstances produce different orders.
fn make_two_class_profile() -> AllocationProfile {
    AllocationProfile {
        members: vec![
            // ClassA: bigger in bytes, fewer instances
            ClassHeapStats {
                class_name: "ClassA".to_string(),
                library_uri: None,
                new_space_instances: 10,
                new_space_size: 1_000_000,
                old_space_instances: 5,
                old_space_size: 500_000,
            },
            // ClassB: smaller in bytes, more instances
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

/// Collect all cell text from the buffer into a single string.
fn buffer_content(buf: &Buffer, area: Rect) -> String {
    (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (area.x + x, area.y + y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect()
}

#[test]
fn test_allocation_table_sort_by_size_renders_size_indicator() {
    let profile = make_two_class_profile();
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);

    render_allocation_table(Some(&profile), AllocationSortColumn::BySize, area, &mut buf);

    let content = buffer_content(&buf, area);
    // The header should show the sort indicator (▼) near "Shallow Size"
    assert!(
        content.contains('\u{25bc}'),
        "BySize sort should show ▼ indicator in header; content: {content:?}"
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
    // ClassA has larger total_size → should appear first in BySize sort.
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
    // ClassB has more total_instances → should appear first in ByInstances sort.
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
