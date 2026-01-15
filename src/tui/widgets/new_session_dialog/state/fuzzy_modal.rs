//! Fuzzy search modal state

/// Type of fuzzy modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyModalType {
    /// Configuration selection (from LoadedConfigs)
    Config,
    /// Flavor selection (from project + custom)
    Flavor,
}

impl FuzzyModalType {
    /// Get the modal title
    pub fn title(&self) -> &'static str {
        match self {
            Self::Config => "Select Configuration",
            Self::Flavor => "Select Flavor",
        }
    }

    /// Whether custom input is allowed
    pub fn allows_custom(&self) -> bool {
        match self {
            Self::Config => false, // Must select from list
            Self::Flavor => true,  // Can type custom flavor
        }
    }
}

/// State for the fuzzy search modal
#[derive(Debug, Clone)]
pub struct FuzzyModalState {
    /// Type of modal (determines title and behavior)
    pub modal_type: FuzzyModalType,

    /// User's search query
    pub query: String,

    /// All available items (original order)
    pub items: Vec<String>,

    /// Indices of items matching the query (into `items`)
    pub filtered_indices: Vec<usize>,

    /// Currently highlighted index (into `filtered_indices`)
    pub selected_index: usize,

    /// Scroll offset for long lists
    pub scroll_offset: usize,
}

impl FuzzyModalState {
    /// Create a new fuzzy modal state
    pub fn new(modal_type: FuzzyModalType, items: Vec<String>) -> Self {
        let filtered_indices: Vec<usize> = (0..items.len()).collect();
        Self {
            modal_type,
            query: String::new(),
            items,
            filtered_indices,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Get the currently selected item, or the query if no match
    pub fn selected_value(&self) -> Option<String> {
        if let Some(&idx) = self.filtered_indices.get(self.selected_index) {
            // Use safe access to prevent panic if index is somehow invalid
            self.items.get(idx).cloned()
        } else if self.modal_type.allows_custom() && !self.query.is_empty() {
            Some(self.query.clone())
        } else {
            None
        }
    }

    /// Check if there are any filtered results
    pub fn has_results(&self) -> bool {
        !self.filtered_indices.is_empty()
    }

    /// Get the number of filtered results
    pub fn result_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Navigate up in the list
    pub fn navigate_up(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.filtered_indices.len() - 1
            } else {
                self.selected_index - 1
            };
            self.adjust_scroll();
        }
    }

    /// Navigate down in the list
    pub fn navigate_down(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
            self.adjust_scroll();
        }
    }

    /// Adjust scroll offset to keep selection visible
    fn adjust_scroll(&mut self) {
        const VISIBLE_ITEMS: usize = 7; // Number of items visible in modal

        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + VISIBLE_ITEMS {
            self.scroll_offset = self.selected_index - VISIBLE_ITEMS + 1;
        }
    }

    /// Add a character to the query
    pub fn input_char(&mut self, c: char) {
        self.query.push(c);
        self.update_filter();
    }

    /// Remove the last character from the query
    pub fn backspace(&mut self) {
        self.query.pop();
        self.update_filter();
    }

    /// Clear the query
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.update_filter();
    }

    /// Update filtered indices based on current query
    pub fn update_filter(&mut self) {
        use super::super::fuzzy_modal::fuzzy_filter;

        // Reset selection when filter changes
        self.selected_index = 0;
        self.scroll_offset = 0;

        self.filtered_indices = fuzzy_filter(&self.query, &self.items);
    }
}
