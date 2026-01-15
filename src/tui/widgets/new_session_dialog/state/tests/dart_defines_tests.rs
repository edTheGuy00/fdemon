//! Tests for dart defines modal state

use super::super::{DartDefine, DartDefinesEditField, DartDefinesModalState};

#[test]
fn test_dart_defines_modal_new() {
    let defines = vec![
        DartDefine::new("API_KEY", "secret"),
        DartDefine::new("DEBUG", "true"),
    ];
    let state = DartDefinesModalState::new(defines);

    assert_eq!(state.defines.len(), 2);
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.list_item_count(), 3); // 2 defines + Add New
}

#[test]
fn test_navigation_wraps() {
    let defines = vec![DartDefine::new("A", "1")];
    let mut state = DartDefinesModalState::new(defines);

    assert_eq!(state.selected_index, 0);
    state.navigate_down();
    assert_eq!(state.selected_index, 1); // Add New
    state.navigate_down();
    assert_eq!(state.selected_index, 0); // Wrap to first
    state.navigate_up();
    assert_eq!(state.selected_index, 1); // Wrap to Add New
}

#[test]
fn test_load_existing_into_edit() {
    let defines = vec![DartDefine::new("KEY", "value")];
    let mut state = DartDefinesModalState::new(defines);

    state.load_selected_into_edit();

    assert_eq!(state.editing_key, "KEY");
    assert_eq!(state.editing_value, "value");
    assert!(!state.is_new);
    assert_eq!(state.active_pane, super::super::DartDefinesPane::Edit);
}

#[test]
fn test_load_add_new_into_edit() {
    let defines = vec![DartDefine::new("KEY", "value")];
    let mut state = DartDefinesModalState::new(defines);

    state.navigate_down(); // Select Add New
    state.load_selected_into_edit();

    assert_eq!(state.editing_key, "");
    assert_eq!(state.editing_value, "");
    assert!(state.is_new);
}

#[test]
fn test_save_new_define() {
    let mut state = DartDefinesModalState::new(vec![]);

    state.is_new = true;
    state.editing_key = "NEW_KEY".into();
    state.editing_value = "new_value".into();

    assert!(state.save_edit());
    assert_eq!(state.defines.len(), 1);
    assert_eq!(state.defines[0].key, "NEW_KEY");
}

#[test]
fn test_save_empty_key_fails() {
    let mut state = DartDefinesModalState::new(vec![]);

    state.is_new = true;
    state.editing_key = "   ".into(); // Only whitespace

    assert!(!state.save_edit());
    assert!(state.defines.is_empty());
}

#[test]
fn test_delete_define() {
    let defines = vec![DartDefine::new("A", "1"), DartDefine::new("B", "2")];
    let mut state = DartDefinesModalState::new(defines);

    state.selected_index = 0;
    assert!(state.delete_selected());

    assert_eq!(state.defines.len(), 1);
    assert_eq!(state.defines[0].key, "B");
}

#[test]
fn test_cannot_delete_add_new() {
    let defines = vec![DartDefine::new("A", "1")];
    let mut state = DartDefinesModalState::new(defines);

    state.selected_index = 1; // Add New
    assert!(!state.delete_selected());
    assert_eq!(state.defines.len(), 1);
}

#[test]
fn test_edit_field_tab_order() {
    let field = DartDefinesEditField::Key;
    assert_eq!(field.next(), DartDefinesEditField::Value);
    assert_eq!(field.next().next(), DartDefinesEditField::Save);
    assert_eq!(field.next().next().next(), DartDefinesEditField::Delete);
    assert_eq!(field.next().next().next().next(), DartDefinesEditField::Key);
}

#[test]
fn test_delete_middle_item_adjusts_selection() {
    // Test that deleting middle item keeps selection in valid range
    let defines = vec![
        DartDefine::new("A", "1"),
        DartDefine::new("B", "2"),
        DartDefine::new("C", "3"),
    ];
    let mut state = DartDefinesModalState::new(defines);

    // Delete middle item (index 1 = "B")
    state.selected_index = 1;
    assert!(state.delete_selected());

    // After deletion: ["A", "C"], selected_index should be 1 (now "C")
    assert_eq!(state.defines.len(), 2);
    assert_eq!(state.selected_index, 1);
    assert_eq!(state.defines[1].key, "C");
}

#[test]
fn test_delete_last_item_clamps_selection() {
    // Test that deleting last item clamps selection to new last item
    let defines = vec![DartDefine::new("A", "1"), DartDefine::new("B", "2")];
    let mut state = DartDefinesModalState::new(defines);

    // Delete last item (index 1 = "B")
    state.selected_index = 1;
    assert!(state.delete_selected());

    // After deletion: ["A"], selected_index should be 0 (clamped)
    assert_eq!(state.defines.len(), 1);
    assert_eq!(state.selected_index, 0);
    assert_eq!(state.defines[0].key, "A");
}

#[test]
fn test_delete_only_item_points_to_add_new() {
    // Test that deleting the only item leaves selection at 0 (Add New)
    let defines = vec![DartDefine::new("A", "1")];
    let mut state = DartDefinesModalState::new(defines);

    // Delete only item (index 0 = "A")
    state.selected_index = 0;
    assert!(state.delete_selected());

    // After deletion: [], selected_index should be 0 (Add New)
    assert!(state.defines.is_empty());
    assert_eq!(state.selected_index, 0);
    assert!(state.is_add_new_selected());
}
