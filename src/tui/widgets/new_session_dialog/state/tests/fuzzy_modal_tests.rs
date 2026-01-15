//! Tests for fuzzy modal state

use super::super::{FuzzyModalState, FuzzyModalType};

#[test]
fn test_fuzzy_modal_new() {
    let items = vec!["alpha".into(), "beta".into(), "gamma".into()];
    let state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

    assert_eq!(state.query, "");
    assert_eq!(state.filtered_indices.len(), 3);
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_fuzzy_navigation() {
    let items = vec!["a".into(), "b".into(), "c".into()];
    let mut state = FuzzyModalState::new(FuzzyModalType::Config, items);

    assert_eq!(state.selected_index, 0);
    state.navigate_down();
    assert_eq!(state.selected_index, 1);
    state.navigate_down();
    assert_eq!(state.selected_index, 2);
    state.navigate_down(); // Wrap
    assert_eq!(state.selected_index, 0);
    state.navigate_up(); // Wrap back
    assert_eq!(state.selected_index, 2);
}

#[test]
fn test_fuzzy_filter_basic() {
    let items = vec!["dev".into(), "staging".into(), "production".into()];
    let mut state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

    state.input_char('d');
    assert_eq!(state.filtered_indices.len(), 2); // dev, production

    state.input_char('e');
    assert_eq!(state.filtered_indices.len(), 1); // dev only (production doesn't have "de")

    state.input_char('v');
    assert_eq!(state.filtered_indices.len(), 1); // dev only
}

#[test]
fn test_fuzzy_custom_value() {
    let items = vec!["existing".into()];
    let mut state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

    state.input_char('c');
    state.input_char('u');
    state.input_char('s');
    state.input_char('t');
    state.input_char('o');
    state.input_char('m');

    // No matches, but Flavor allows custom
    assert!(!state.has_results());
    assert_eq!(state.selected_value(), Some("custom".into()));
}

#[test]
fn test_config_no_custom() {
    let items = vec!["existing".into()];
    let mut state = FuzzyModalState::new(FuzzyModalType::Config, items);

    state.input_char('z'); // No match - 'z' not in "existing"

    assert!(!state.has_results());
    assert_eq!(state.selected_value(), None); // Config doesn't allow custom
}
