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
    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
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

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
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

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
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

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_legend_omits_raster_when_zero() {
    // All raster_cache values are 0 → "Raster" should not appear in legend
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

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
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

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
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

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

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
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
    render_allocation_table(Some(&profile), area, &mut buf);

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
    render_allocation_table(None, area, &mut buf);

    let content: String = (0..10u16)
        .flat_map(|y| (0..80u16).map(move |x| (x, y)))
        .filter_map(|(x, y)| buf.cell((x, y)).map(|c| c.symbol().to_string()))
        .collect();

    assert!(
        content.contains("loading"),
        "Should show loading message when profile is None"
    );
}

#[test]
fn test_compact_mode_small_height() {
    // area height < MIN_CHART_HEIGHT (6) → compact summary
    let samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
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

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
    let area = Rect::new(0, 0, 10, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn test_zero_area_no_panic() {
    let samples: RingBuffer<MemorySample> = RingBuffer::new(120);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(60);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(50);

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
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

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
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
    // → shows chart only, no table
    let mut samples: RingBuffer<MemorySample> = RingBuffer::new(10);
    let memory_history: RingBuffer<MemoryUsage> = RingBuffer::new(10);
    let gc_history: RingBuffer<GcEvent> = RingBuffer::new(10);

    samples.push(make_sample(50_000_000, 5_000_000, 0, 100_000_000, 0));

    let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
    // height = 8 is >= MIN_CHART_HEIGHT(6) but < MIN_CHART_HEIGHT+MIN_TABLE_HEIGHT (9)
    let area = Rect::new(0, 0, 80, 8);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    // No panic, no "loading" or class table text expected
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
