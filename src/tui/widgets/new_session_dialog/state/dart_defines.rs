//! Dart defines modal state

/// A single dart define key-value pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartDefine {
    pub key: String,
    pub value: String,
}

impl DartDefine {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Format as command line argument
    pub fn to_arg(&self) -> String {
        format!("{}={}", self.key, self.value)
    }
}

/// Which pane is focused in the dart defines modal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DartDefinesPane {
    #[default]
    List,
    Edit,
}

/// Which field is focused in the edit pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DartDefinesEditField {
    #[default]
    Key,
    Value,
    Save,
    Delete,
}

impl DartDefinesEditField {
    /// Get next field in tab order
    pub fn next(self) -> Self {
        match self {
            Self::Key => Self::Value,
            Self::Value => Self::Save,
            Self::Save => Self::Delete,
            Self::Delete => Self::Key,
        }
    }

    /// Get previous field in tab order
    pub fn prev(self) -> Self {
        match self {
            Self::Key => Self::Delete,
            Self::Value => Self::Key,
            Self::Save => Self::Value,
            Self::Delete => Self::Save,
        }
    }
}

/// State for the dart defines modal
#[derive(Debug, Clone)]
pub struct DartDefinesModalState {
    /// All dart defines (working copy)
    pub defines: Vec<DartDefine>,

    /// Currently selected index in the list (includes "[+] Add New" at end)
    pub selected_index: usize,

    /// Scroll offset for long lists
    pub scroll_offset: usize,

    /// Which pane is currently focused
    pub active_pane: DartDefinesPane,

    /// Which field is focused in the edit pane
    pub edit_field: DartDefinesEditField,

    /// Current value in the Key input field
    pub editing_key: String,

    /// Current value in the Value input field
    pub editing_value: String,

    /// Whether we're editing a new define (vs existing)
    pub is_new: bool,
}

impl DartDefinesModalState {
    /// Create a new dart defines modal state from existing defines
    pub fn new(defines: Vec<DartDefine>) -> Self {
        Self {
            defines,
            selected_index: 0,
            scroll_offset: 0,
            active_pane: DartDefinesPane::List,
            edit_field: DartDefinesEditField::Key,
            editing_key: String::new(),
            editing_value: String::new(),
            is_new: false,
        }
    }

    /// Check if the "[+] Add New" option is selected
    pub fn is_add_new_selected(&self) -> bool {
        self.selected_index >= self.defines.len()
    }

    /// Get the currently selected define (if any)
    pub fn selected_define(&self) -> Option<&DartDefine> {
        self.defines.get(self.selected_index)
    }

    /// Get the total number of items in list (defines + Add New)
    pub fn list_item_count(&self) -> usize {
        self.defines.len() + 1 // +1 for "[+] Add New"
    }

    /// Navigate up in the list
    pub fn navigate_up(&mut self) {
        if self.list_item_count() > 0 {
            self.selected_index = if self.selected_index == 0 {
                self.list_item_count() - 1
            } else {
                self.selected_index - 1
            };
            self.adjust_scroll();
        }
    }

    /// Navigate down in the list
    pub fn navigate_down(&mut self) {
        if self.list_item_count() > 0 {
            self.selected_index = (self.selected_index + 1) % self.list_item_count();
            self.adjust_scroll();
        }
    }

    /// Adjust scroll offset to keep selection visible
    fn adjust_scroll(&mut self) {
        const VISIBLE_ITEMS: usize = 10;

        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + VISIBLE_ITEMS {
            self.scroll_offset = self.selected_index - VISIBLE_ITEMS + 1;
        }
    }

    /// Switch to the other pane
    pub fn switch_pane(&mut self) {
        self.active_pane = match self.active_pane {
            DartDefinesPane::List => DartDefinesPane::Edit,
            DartDefinesPane::Edit => DartDefinesPane::List,
        };
    }

    /// Move to next field in edit pane
    pub fn next_field(&mut self) {
        self.edit_field = self.edit_field.next();
    }

    /// Move to previous field in edit pane
    pub fn prev_field(&mut self) {
        self.edit_field = self.edit_field.prev();
    }

    /// Load the selected define into the edit form
    pub fn load_selected_into_edit(&mut self) {
        // Clone the selected define to avoid borrow checker issues
        let selected = self.defines.get(self.selected_index).cloned();

        if let Some(define) = selected {
            self.editing_key = define.key;
            self.editing_value = define.value;
            self.is_new = false;
        } else {
            // "[+] Add New" selected
            self.editing_key.clear();
            self.editing_value.clear();
            self.is_new = true;
        }
        self.active_pane = DartDefinesPane::Edit;
        self.edit_field = DartDefinesEditField::Key;
    }

    /// Save the current edit form to the defines list
    /// Returns true if save was successful
    pub fn save_edit(&mut self) -> bool {
        // Validate: key cannot be empty
        if self.editing_key.trim().is_empty() {
            return false;
        }

        let define = DartDefine::new(
            self.editing_key.trim().to_string(),
            self.editing_value.clone(),
        );

        if self.is_new {
            // Add new define
            self.defines.push(define);
            self.selected_index = self.defines.len() - 1;
            self.is_new = false;
        } else {
            // Update existing
            if let Some(existing) = self.defines.get_mut(self.selected_index) {
                *existing = define;
            }
        }

        true
    }

    /// Delete the currently selected define
    /// Returns true if delete was performed
    pub fn delete_selected(&mut self) -> bool {
        if self.is_add_new_selected() {
            return false; // Can't delete "[+] Add New"
        }

        if self.selected_index < self.defines.len() {
            self.defines.remove(self.selected_index);

            // Adjust selection: clamp to valid range after removal
            // Note: saturating_sub(1) on 0 returns 0, so this handles empty list correctly
            // (index 0 will point to "[+] Add New")
            if self.selected_index >= self.defines.len() {
                self.selected_index = self.defines.len().saturating_sub(1);
            }

            // Clear edit form
            self.editing_key.clear();
            self.editing_value.clear();

            // Return to list
            self.active_pane = DartDefinesPane::List;

            return true;
        }

        false
    }

    /// Input a character to the currently focused text field
    pub fn input_char(&mut self, c: char) {
        match self.edit_field {
            DartDefinesEditField::Key => self.editing_key.push(c),
            DartDefinesEditField::Value => self.editing_value.push(c),
            _ => {}
        }
    }

    /// Backspace in the currently focused text field
    pub fn backspace(&mut self) {
        match self.edit_field {
            DartDefinesEditField::Key => {
                self.editing_key.pop();
            }
            DartDefinesEditField::Value => {
                self.editing_value.pop();
            }
            _ => {}
        }
    }

    /// Check if there are unsaved changes in the edit form
    pub fn has_unsaved_changes(&self) -> bool {
        if self.is_new {
            !self.editing_key.is_empty() || !self.editing_value.is_empty()
        } else if let Some(define) = self.selected_define() {
            self.editing_key != define.key || self.editing_value != define.value
        } else {
            false
        }
    }
}
