use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table,
};
use ratatui::Frame;
use tracing::debug;

use loom_core::entry::LdapEntry;
use loom_core::filter::{detect_filter_context, validate_filter, FilterContext};
use loom_core::schema::SchemaCache;

use crate::action::Action;
use crate::component::Component;
use crate::theme::Theme;
use crate::widgets::fuzzy_input::{FuzzyFilter, FuzzyMatch};

/// Well-known LDAP attribute names used as a fallback when schema loading fails.
const COMMON_ATTRIBUTES: &[&str] = &[
    "cn",
    "sn",
    "givenName",
    "displayName",
    "mail",
    "uid",
    "userPrincipalName",
    "sAMAccountName",
    "memberOf",
    "member",
    "objectClass",
    "description",
    "telephoneNumber",
    "title",
    "department",
    "company",
    "manager",
    "distinguishedName",
    "name",
    "ou",
    "dc",
    "o",
    "c",
    "l",
    "st",
    "postalCode",
    "streetAddress",
    "userPassword",
    "unicodePwd",
    "accountExpires",
    "pwdLastSet",
    "lastLogon",
    "whenCreated",
    "whenChanged",
    "objectGUID",
    "objectSid",
];

/// Filter templates shown when buffer is empty or just `(`.
const FILTER_TEMPLATES: &[&str] = &[
    "(objectClass=*)",
    "(&(objectClass=person)(cn=*))",
    "(&(objectClass=group)(cn=*))",
    "(&(objectClass=user)(sAMAccountName=*))",
    "(|(cn=*)(sn=*))",
];

/// Which kind of completions are currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionKind {
    Attributes,
    Values,
    Templates,
}

/// The bottom-right panel: command input and status messages.
pub struct CommandPanel {
    pub messages: Vec<StatusMessage>,
    /// When true, the input line is active and capturing keystrokes.
    pub input_active: bool,
    pub input_buffer: String,
    theme: Theme,
    area: Option<Rect>,

    // Config flags
    autocomplete_enabled: bool,
    live_search_enabled: bool,

    // Cursor position within input_buffer
    pub cursor_pos: usize,

    // Schema for value suggestions
    schema: Option<SchemaCache>,

    // Autocomplete state
    attribute_names: Vec<String>,
    fuzzy: FuzzyFilter,
    completions: Vec<FuzzyMatch>,
    completion_visible: bool,
    completion_selected: usize,
    completion_kind: CompletionKind,
    value_items: Vec<String>,

    // Live search debounce state
    search_generation: u64,
    search_dirty: bool,
    last_search_text: String,
    live_searching: bool,

    // Inline preview of live search results
    preview_results: Vec<LdapEntry>,
    preview_label: String,
}

pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

impl CommandPanel {
    pub fn new(theme: Theme, autocomplete_enabled: bool, live_search_enabled: bool) -> Self {
        Self {
            messages: Vec::new(),
            input_active: false,
            input_buffer: String::new(),
            theme,
            area: None,
            autocomplete_enabled,
            live_search_enabled,
            cursor_pos: 0,
            schema: None,
            attribute_names: Vec::new(),
            fuzzy: FuzzyFilter::new(),
            completions: Vec::new(),
            completion_visible: false,
            completion_selected: 0,
            completion_kind: CompletionKind::Attributes,
            value_items: Vec::new(),
            search_generation: 0,
            search_dirty: false,
            last_search_text: String::new(),
            live_searching: false,
            preview_results: Vec::new(),
            preview_label: String::new(),
        }
    }

    pub fn push_message(&mut self, text: String) {
        self.messages.push(StatusMessage {
            text,
            is_error: false,
        });
        // Keep last 100 messages
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    pub fn push_error(&mut self, text: String) {
        self.messages.push(StatusMessage {
            text,
            is_error: true,
        });
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    pub fn activate_input(&mut self) {
        self.input_active = true;
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.search_dirty = false;
        self.last_search_text.clear();
        self.live_searching = false;
        self.hide_completions();
    }

    pub fn deactivate_input(&mut self) {
        self.input_active = false;
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.search_dirty = false;
        self.last_search_text.clear();
        self.live_searching = false;
        self.hide_completions();
        self.clear_preview();
    }

    /// Deactivate input but preserve the buffer so the filter text remains
    /// visible and editable on resume.
    pub fn soft_deactivate(&mut self) {
        self.input_active = false;
        self.completion_visible = false;
        self.live_searching = false;
        self.preview_results.clear();
        self.preview_label.clear();
        // Keep input_buffer, cursor_pos, search state intact
    }

    /// Reactivate input with existing buffer content.
    pub fn resume_input(&mut self) {
        self.input_active = true;
        self.cursor_pos = self.input_buffer.len();
        self.update_completions();
    }

    /// Set the attribute names available for autocomplete.
    pub fn set_attribute_names(&mut self, names: Vec<String>) {
        debug!(
            "set_attribute_names: received {} attribute names (first 10: {:?})",
            names.len(),
            &names[..names.len().min(10)]
        );
        self.attribute_names = names;
    }

    /// Set the schema cache for value suggestions.
    pub fn set_schema(&mut self, schema: Option<SchemaCache>) {
        debug!(
            "set_schema: schema={}, attr_types={}, obj_classes={}",
            schema.is_some(),
            schema.as_ref().map_or(0, |s| s.attribute_types.len()),
            schema.as_ref().map_or(0, |s| s.object_classes.len()),
        );
        self.schema = schema;
    }

    /// Populate attribute_names with well-known LDAP attributes as a fallback
    /// when schema loading fails. Only sets them if attribute_names is currently empty.
    pub fn set_fallback_attributes(&mut self) {
        if self.attribute_names.is_empty() {
            debug!(
                "set_fallback_attributes: populating with {} common attributes",
                COMMON_ATTRIBUTES.len()
            );
            self.attribute_names = COMMON_ATTRIBUTES.iter().map(|s| s.to_string()).collect();
        } else {
            debug!(
                "set_fallback_attributes: skipped, already have {} attributes",
                self.attribute_names.len()
            );
        }
    }

    fn hide_completions(&mut self) {
        self.completion_visible = false;
        self.completions.clear();
        self.completion_selected = 0;
        self.value_items.clear();
    }

    fn update_completions(&mut self) {
        if !self.autocomplete_enabled {
            debug!("update_completions: autocomplete disabled, hiding");
            self.hide_completions();
            return;
        }

        // Use text up to cursor for context detection — the auto-paren feature
        // inserts a closing ')' after the cursor which would make balanced parens
        // look "complete" and hide completions prematurely.
        let text_to_cursor = &self.input_buffer[..self.cursor_pos];
        let context = detect_filter_context(text_to_cursor);
        debug!(
            "update_completions: buffer={:?}, cursor_pos={}, text_to_cursor={:?}, context={:?}, attribute_names_count={}, has_schema={}",
            self.input_buffer,
            self.cursor_pos,
            text_to_cursor,
            context,
            self.attribute_names.len(),
            self.schema.is_some(),
        );

        match context {
            Some(FilterContext::Empty) => {
                // Show filter templates
                debug!(
                    "update_completions: showing {} filter templates",
                    FILTER_TEMPLATES.len()
                );
                self.completion_kind = CompletionKind::Templates;
                self.value_items = FILTER_TEMPLATES.iter().map(|s| s.to_string()).collect();
                self.completions = self
                    .value_items
                    .iter()
                    .enumerate()
                    .map(|(i, _)| FuzzyMatch { index: i, score: 0 })
                    .collect();
                self.completion_visible = !self.completions.is_empty();
                if self.completion_selected >= self.completions.len() {
                    self.completion_selected = 0;
                }
            }
            Some(FilterContext::AttributeName { partial }) => {
                if self.attribute_names.is_empty() {
                    debug!(
                        "update_completions: AttributeName context partial={:?} but attribute_names is EMPTY, hiding completions",
                        partial
                    );
                    self.hide_completions();
                    return;
                }
                self.completion_kind = CompletionKind::Attributes;
                self.value_items.clear();
                self.completions = self.fuzzy.filter(&partial, &self.attribute_names);
                debug!(
                    "update_completions: AttributeName partial={:?}, fuzzy matched {} of {} attributes",
                    partial,
                    self.completions.len(),
                    self.attribute_names.len(),
                );
                if !self.completions.is_empty() {
                    let top_matches: Vec<&str> = self
                        .completions
                        .iter()
                        .take(5)
                        .map(|m| self.attribute_names[m.index].as_str())
                        .collect();
                    debug!("update_completions: top matches: {:?}", top_matches);
                }
                self.completions.truncate(50);
                self.completion_visible = !self.completions.is_empty();
                if self.completion_selected >= self.completions.len() {
                    self.completion_selected = 0;
                }
            }
            Some(FilterContext::Value { attr, partial }) => {
                self.completion_kind = CompletionKind::Values;
                self.value_items.clear();

                let attr_lower = attr.to_lowercase();

                if attr_lower == "objectclass" {
                    // Suggest object class names from schema
                    if let Some(schema) = &self.schema {
                        self.value_items = schema.object_classes.keys().cloned().collect();
                        debug!(
                            "update_completions: Value context for objectClass, {} object classes from schema",
                            self.value_items.len()
                        );
                    } else {
                        debug!("update_completions: Value context for objectClass but no schema available");
                    }
                }

                // Only show value completions if we have meaningful suggestions
                // (e.g. objectClass names from schema). Don't show generic
                // placeholders like *, TRUE, FALSE — they're not helpful.
                if self.value_items.is_empty() {
                    debug!(
                        "update_completions: Value attr={:?}, no value suggestions available, hiding",
                        attr
                    );
                    self.hide_completions();
                } else {
                    // Filter by partial
                    if partial.is_empty() {
                        self.completions = self
                            .value_items
                            .iter()
                            .enumerate()
                            .map(|(i, _)| FuzzyMatch { index: i, score: 0 })
                            .collect();
                    } else {
                        self.completions = self.fuzzy.filter(&partial, &self.value_items);
                    }
                    debug!(
                        "update_completions: Value attr={:?}, partial={:?}, {} value completions",
                        attr,
                        partial,
                        self.completions.len()
                    );
                    self.completions.truncate(50);
                    self.completion_visible = !self.completions.is_empty();
                    if self.completion_selected >= self.completions.len() {
                        self.completion_selected = 0;
                    }
                }
            }
            None => {
                debug!("update_completions: no filter context detected, hiding completions");
                self.hide_completions();
            }
        }
    }

    fn accept_completion(&mut self) {
        if !self.completion_visible || self.completions.is_empty() {
            debug!(
                "accept_completion: nothing to accept (visible={}, count={})",
                self.completion_visible,
                self.completions.len()
            );
            return;
        }

        let selected = &self.completions[self.completion_selected];
        debug!(
            "accept_completion: kind={:?}, selected_idx={}, item_idx={}",
            self.completion_kind, self.completion_selected, selected.index
        );

        // Use text up to cursor for context detection (same as update_completions)
        let text_to_cursor = &self.input_buffer[..self.cursor_pos];

        match self.completion_kind {
            CompletionKind::Attributes => {
                let attr_name = self.attribute_names[selected.index].clone();
                debug!("accept_completion: accepting attribute {:?}", attr_name);
                if let Some(FilterContext::AttributeName { partial }) =
                    detect_filter_context(text_to_cursor)
                {
                    // Replace the partial (before cursor) with full name + '=',
                    // preserving any text after the cursor (e.g. auto-inserted ')')
                    let prefix_end = self.cursor_pos - partial.len();
                    let suffix = self.input_buffer[self.cursor_pos..].to_string();
                    self.input_buffer.truncate(prefix_end);
                    self.input_buffer.push_str(&attr_name);
                    self.input_buffer.push('=');
                    self.cursor_pos = self.input_buffer.len();
                    self.input_buffer.push_str(&suffix);
                    debug!(
                        "accept_completion: buffer is now {:?}, cursor_pos={}",
                        self.input_buffer, self.cursor_pos
                    );
                }
            }
            CompletionKind::Values => {
                let value = self.value_items[selected.index].clone();
                debug!("accept_completion: accepting value {:?}", value);
                if let Some(FilterContext::Value { partial, .. }) =
                    detect_filter_context(text_to_cursor)
                {
                    // Replace the partial (before cursor) with full value + ')',
                    // consuming an existing ')' after cursor if present (from auto-paren)
                    let prefix_end = self.cursor_pos - partial.len();
                    let suffix = &self.input_buffer[self.cursor_pos..];
                    // Skip the auto-inserted ')' since we're adding our own
                    let suffix = if let Some(stripped) = suffix.strip_prefix(')') {
                        stripped.to_string()
                    } else {
                        suffix.to_string()
                    };
                    self.input_buffer.truncate(prefix_end);
                    self.input_buffer.push_str(&value);
                    self.input_buffer.push(')');
                    self.cursor_pos = self.input_buffer.len();
                    self.input_buffer.push_str(&suffix);
                    debug!(
                        "accept_completion: buffer is now {:?}, cursor_pos={}",
                        self.input_buffer, self.cursor_pos
                    );
                }
            }
            CompletionKind::Templates => {
                let template = self.value_items[selected.index].clone();
                debug!("accept_completion: accepting template {:?}", template);
                self.input_buffer = template;
                self.cursor_pos = self.input_buffer.len();
            }
        }

        self.hide_completions();
        // Trigger completions update for the new buffer state
        self.update_completions();
    }

    /// Tick-based debounce for live search.
    /// Called by App on Action::Tick when input is active.
    /// Returns a LiveSearchRequest if the filter changed and is valid.
    pub fn tick(&mut self) -> Action {
        if !self.live_search_enabled || !self.search_dirty {
            return Action::None;
        }

        if self.input_buffer == self.last_search_text {
            return Action::None;
        }

        if self.input_buffer.is_empty() {
            return Action::None;
        }

        let validation = validate_filter(&self.input_buffer);
        if validation.is_ok() {
            self.search_dirty = false;
            self.last_search_text = self.input_buffer.clone();
            self.search_generation += 1;
            self.live_searching = true;

            debug!(
                "tick: emitting LiveSearchRequest gen={} filter={:?}",
                self.search_generation, self.input_buffer
            );

            Action::LiveSearchRequest {
                generation: self.search_generation,
                filter: self.input_buffer.clone(),
            }
        } else {
            debug!(
                "tick: filter {:?} invalid: {:?}",
                self.input_buffer,
                validation.err()
            );
            Action::None
        }
    }

    /// Check if live search results are still fresh (generation matches).
    /// Returns true if the results should be displayed.
    pub fn receive_live_results(&mut self, generation: u64) -> bool {
        if generation == self.search_generation {
            self.live_searching = false;
            true
        } else {
            false
        }
    }

    /// Store live search results for inline preview rendering.
    pub fn set_preview_results(&mut self, label: String, results: Vec<LdapEntry>) {
        self.preview_label = label;
        self.preview_results = results;
    }

    /// Clear the inline preview results.
    pub fn clear_preview(&mut self) {
        self.preview_results.clear();
        self.preview_label.clear();
    }

    /// Handle key events when the command panel is focused.
    /// Returns an Action for the app to dispatch.
    pub fn handle_input_key(&mut self, key: KeyEvent) -> Action {
        if !self.input_active {
            // Activate on '/' or ':'
            match key.code {
                KeyCode::Char('/') | KeyCode::Char(':') => {
                    self.activate_input();
                    // Show template completions immediately
                    self.update_completions();
                    return Action::None;
                }
                _ => return Action::None,
            }
        }

        // When completions are visible, intercept some keys
        if self.completion_visible {
            match key.code {
                KeyCode::Tab => {
                    self.accept_completion();
                    return Action::None;
                }
                KeyCode::Down => {
                    if !self.completions.is_empty() {
                        self.completion_selected =
                            (self.completion_selected + 1) % self.completions.len();
                    }
                    return Action::None;
                }
                KeyCode::Up => {
                    if !self.completions.is_empty() {
                        self.completion_selected = if self.completion_selected == 0 {
                            self.completions.len() - 1
                        } else {
                            self.completion_selected - 1
                        };
                    }
                    return Action::None;
                }
                KeyCode::Esc => {
                    self.hide_completions();
                    return Action::None;
                }
                // Enter, Char, Backspace fall through to normal handling
                _ => {}
            }
        }

        // Input mode: capture text
        match key.code {
            KeyCode::Enter => {
                let query = normalize_filter(&self.input_buffer);
                self.soft_deactivate();
                if query.is_empty() {
                    Action::None
                } else {
                    Action::SearchExecute(query)
                }
            }
            KeyCode::Esc => {
                self.soft_deactivate();
                Action::None
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    let removed = self.input_buffer.remove(self.cursor_pos);
                    // If we deleted a '(' and the char now at cursor_pos is ')',
                    // remove the matching ')' too.
                    if removed == '('
                        && self.cursor_pos < self.input_buffer.len()
                        && self.input_buffer.as_bytes()[self.cursor_pos] == b')'
                    {
                        self.input_buffer.remove(self.cursor_pos);
                    }
                    // If we deleted a ')' and the char before cursor is '(' with
                    // no content between them, remove the orphaned '(' too.
                    if removed == ')'
                        && self.cursor_pos > 0
                        && self.input_buffer.as_bytes()[self.cursor_pos - 1] == b'('
                    {
                        self.cursor_pos -= 1;
                        self.input_buffer.remove(self.cursor_pos);
                    }
                    self.search_dirty = true;
                    self.clear_preview();
                    self.update_completions();
                }
                Action::None
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.input_buffer.len() {
                    let removed = self.input_buffer.remove(self.cursor_pos);
                    // If we deleted '(' and the char now at cursor_pos is ')',
                    // remove the matching ')' too.
                    if removed == '('
                        && self.cursor_pos < self.input_buffer.len()
                        && self.input_buffer.as_bytes()[self.cursor_pos] == b')'
                    {
                        self.input_buffer.remove(self.cursor_pos);
                    }
                    // If we deleted ')' and the char before cursor is '(' with
                    // no content between them, remove the orphaned '(' too.
                    if removed == ')'
                        && self.cursor_pos > 0
                        && self.input_buffer.as_bytes()[self.cursor_pos - 1] == b'('
                    {
                        self.cursor_pos -= 1;
                        self.input_buffer.remove(self.cursor_pos);
                    }
                    self.search_dirty = true;
                    self.clear_preview();
                    self.update_completions();
                }
                Action::None
            }
            KeyCode::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                Action::None
            }
            KeyCode::Right => {
                if self.cursor_pos < self.input_buffer.len() {
                    self.cursor_pos += 1;
                }
                Action::None
            }
            KeyCode::Up => {
                self.move_cursor_vertical(-1);
                Action::None
            }
            KeyCode::Down => {
                self.move_cursor_vertical(1);
                Action::None
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
                Action::None
            }
            KeyCode::End => {
                self.cursor_pos = self.input_buffer.len();
                Action::None
            }
            KeyCode::Char(c) => {
                if c == '(' && self.autocomplete_enabled {
                    // Auto-insert matching parentheses
                    self.input_buffer.insert(self.cursor_pos, '(');
                    self.input_buffer.insert(self.cursor_pos + 1, ')');
                    self.cursor_pos += 1; // position between ( and )
                } else {
                    self.input_buffer.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;

                    // Auto-wrap bare attr= in parentheses:
                    // If user typed '=' and there are no parens yet, wrap in (...)
                    if c == '=' && !self.input_buffer.contains('(') {
                        self.input_buffer.insert(0, '(');
                        self.input_buffer.push(')');
                        self.cursor_pos += 1; // account for prepended '('
                    }
                }
                self.search_dirty = true;
                self.clear_preview();
                self.update_completions();
                Action::None
            }
            _ => Action::None,
        }
    }
}

/// Normalize a filter string before submission:
/// - Bare filter (no parens): wrap in `()` — e.g. `cn=test` → `(cn=test)`
/// - Multiple top-level `(...)` groups: wrap in `(&...)` — e.g. `(cn=r)(mail=r)` → `(&(cn=r)(mail=r))`
/// - Otherwise: return as-is
fn normalize_filter(filter: &str) -> String {
    let trimmed = filter.trim();
    if trimmed.is_empty() {
        return trimmed.to_string();
    }

    // Count top-level parenthesized groups
    let mut depth: i32 = 0;
    let mut top_level_groups = 0;
    let mut has_parens = false;

    for ch in trimmed.chars() {
        match ch {
            '(' => {
                if depth == 0 {
                    top_level_groups += 1;
                }
                depth += 1;
                has_parens = true;
            }
            ')' => {
                depth -= 1;
            }
            _ => {}
        }
    }

    if !has_parens {
        // Bare filter with no parens at all
        format!("({})", trimmed)
    } else if top_level_groups > 1 {
        // Multiple top-level groups — wrap in (&...)
        format!("(&{})", trimmed)
    } else {
        trimmed.to_string()
    }
}

impl CommandPanel {
    /// Format the input buffer for multi-line display when it contains boolean operators.
    /// Returns `(formatted_lines, cursor_row, cursor_col)`.
    /// For simple filters, returns a single line with direct cursor mapping.
    /// For compound filters (`(&`, `(|`, `(!`), formats across multiple indented lines.
    fn move_cursor_vertical(&mut self, delta: i32) {
        let (lines, cursor_row, cursor_col, cursor_map) = self.format_input_for_display();

        if lines.len() <= 1 {
            return;
        }

        let target_row = (cursor_row as i32 + delta).clamp(0, (lines.len() - 1) as i32) as usize;
        if target_row == cursor_row {
            return;
        }

        // Collect all (buf_idx, col) pairs on the target row
        let candidates: Vec<(usize, usize)> = cursor_map
            .iter()
            .enumerate()
            .filter(|&(_, &(row, _))| row == target_row)
            .map(|(buf_idx, &(_, col))| (buf_idx, col))
            .collect();

        if candidates.is_empty() {
            return;
        }

        // Find the candidate with the closest column to cursor_col
        let &(best_idx, _) = candidates
            .iter()
            .min_by_key(|&&(_, col)| (col as i32 - cursor_col as i32).unsigned_abs())
            .unwrap();

        // If cursor_col is beyond the last char on the target row, position after it
        let &(last_idx, last_col) = candidates.last().unwrap();
        if cursor_col > last_col {
            self.cursor_pos = last_idx + 1;
        } else {
            self.cursor_pos = best_idx;
        }
    }

    pub fn format_input_for_display(&self) -> (Vec<String>, usize, usize, Vec<(usize, usize)>) {
        let buf = &self.input_buffer;

        // Check if buffer contains boolean operators
        let has_boolean = buf.contains("(&") || buf.contains("(|") || buf.contains("(!");
        if !has_boolean {
            return (vec![buf.clone()], 0, self.cursor_pos, Vec::new());
        }

        // Build formatted lines and a cursor_map: Vec<(row, col)> for each buffer index
        let mut lines: Vec<String> = Vec::new();
        let mut cursor_map: Vec<(usize, usize)> = Vec::new(); // one entry per buffer char
        let mut current_line = String::new();
        let mut indent: usize = 0;
        let indent_str = "  ";

        let chars: Vec<char> = buf.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let ch = chars[i];

            if ch == '('
                && i + 1 < chars.len()
                && (chars[i + 1] == '&' || chars[i + 1] == '|' || chars[i + 1] == '!')
            {
                // Start of boolean group: emit current line if non-empty, start new line
                if !current_line.trim().is_empty() {
                    lines.push(current_line);
                }
                // Emit "(&" / "(|" / "(!" on its own line
                current_line = format!("{}{}{}", indent_str.repeat(indent), ch, chars[i + 1]);
                let row = lines.len();
                let col_base = indent * indent_str.len();
                cursor_map.push((row, col_base)); // '('
                cursor_map.push((row, col_base + 1)); // '&'/'|'/'!'
                lines.push(current_line);
                current_line = String::new();
                indent += 1;
                i += 2;
            } else if ch == '(' {
                // Start of a child filter — start a new line if current line has content
                if !current_line.trim().is_empty() {
                    lines.push(current_line);
                }
                current_line = format!("{}{}", indent_str.repeat(indent), ch);
                let row = lines.len();
                let col = indent * indent_str.len();
                cursor_map.push((row, col));
                i += 1;
            } else if ch == ')' {
                // Check if this closes a boolean group (current_line is empty or whitespace-only,
                // meaning the children have been emitted already)
                if current_line.trim().is_empty() && indent > 0 {
                    // Closing paren for boolean group — dedent
                    indent -= 1;
                    current_line = format!("{})", indent_str.repeat(indent));
                    let row = lines.len();
                    let col = indent * indent_str.len();
                    cursor_map.push((row, col));
                    lines.push(current_line);
                    current_line = String::new();
                } else {
                    // Closing paren for a child filter — stays on same line
                    let row = lines.len();
                    let col = current_line.len();
                    cursor_map.push((row, col));
                    current_line.push(ch);
                    // End this child line
                    lines.push(current_line);
                    current_line = String::new();
                }
                i += 1;
            } else {
                // Regular character — add to current line
                if current_line.is_empty() {
                    current_line = indent_str.repeat(indent).to_string();
                }
                let row = lines.len();
                let col = current_line.len();
                cursor_map.push((row, col));
                current_line.push(ch);
                i += 1;
            }
        }

        // Flush any remaining content
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // If lines is empty, return a single empty line
        if lines.is_empty() {
            return (vec![String::new()], 0, 0, cursor_map);
        }

        // Map cursor position to (row, col) in formatted output
        let (cursor_row, cursor_col) = if self.cursor_pos < cursor_map.len() {
            cursor_map[self.cursor_pos]
        } else if let Some(&(last_row, last_col)) = cursor_map.last() {
            // Cursor is at end of buffer — position after last char
            (last_row, last_col + 1)
        } else {
            (0, 0)
        };

        (lines, cursor_row, cursor_col, cursor_map)
    }

    /// Render as a read-only status log with a custom title.
    pub fn render_status(&self, frame: &mut Frame, area: Rect, title: &str) {
        let block = Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(self.theme.border);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let msg_height = inner.height as usize;
        let start = self.messages.len().saturating_sub(msg_height);
        let visible_messages = &self.messages[start..];

        let lines: Vec<Line> = visible_messages
            .iter()
            .map(|msg| {
                let style = if msg.is_error {
                    self.theme.error
                } else {
                    self.theme.normal
                };
                Line::from(Span::styled(&msg.text, style))
            })
            .collect();

        frame.render_widget(Paragraph::new(lines), inner);
    }

    /// Render the autocomplete popup above the command panel.
    fn render_completion_popup(&self, frame: &mut Frame, area: Rect) {
        if !self.completion_visible || self.completions.is_empty() {
            return;
        }

        let max_visible = 8;
        let visible_count = self.completions.len().min(max_visible);
        let popup_height = visible_count as u16 + 2; // +2 for border
        let popup_width = 45u16.min(area.width.saturating_sub(2));

        // Position above the command panel input line
        if area.y < popup_height {
            return; // Not enough room above
        }

        let popup_area = Rect {
            x: area.x + 2, // Indent past "/ " prompt
            y: area.y - popup_height,
            width: popup_width,
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);

        let total = self.completions.len();
        let kind_label = match self.completion_kind {
            CompletionKind::Attributes => "Attributes",
            CompletionKind::Values => "Values",
            CompletionKind::Templates => "Templates",
        };
        let title = if total > max_visible {
            format!(" {} ({}/{}) ", kind_label, visible_count, total)
        } else {
            format!(" {} ({}) ", kind_label, total)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border);

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        // Scroll to keep selection visible
        let scroll_offset = if self.completion_selected >= max_visible {
            self.completion_selected - max_visible + 1
        } else {
            0
        };

        // Choose the correct source list based on completion kind
        let source_list: &[String] = match self.completion_kind {
            CompletionKind::Attributes => &self.attribute_names,
            CompletionKind::Values | CompletionKind::Templates => &self.value_items,
        };

        let items: Vec<ListItem> = self.completions[scroll_offset..]
            .iter()
            .take(max_visible)
            .enumerate()
            .map(|(i, m)| {
                let name = &source_list[m.index];
                let actual_idx = scroll_offset + i;
                let style = if actual_idx == self.completion_selected {
                    self.theme.selected
                } else {
                    self.theme.normal
                };
                ListItem::new(Span::styled(name.as_str(), style))
            })
            .collect();

        let list = List::new(items);
        frame.render_widget(list, inner);
    }

    /// Render a non-interactive preview table of live search results above the command panel.
    fn render_preview_overlay(&self, frame: &mut Frame, area: Rect) {
        if self.preview_results.is_empty() {
            return;
        }

        let max_rows = 10;
        let visible_count = self.preview_results.len().min(max_rows);
        let popup_height = visible_count as u16 + 3; // +2 border, +1 header row
        let popup_width = area.width.saturating_sub(2).min(120);

        if area.y < popup_height {
            return; // Not enough room above
        }

        let popup_area = Rect {
            x: area.x + 1,
            y: area.y - popup_height,
            width: popup_width,
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);

        let title = format!(" {} ", self.preview_label);
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border);

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        let header = Row::new(vec![
            Cell::from(Span::styled("DN", self.theme.header)),
            Cell::from(Span::styled("sAMAccountName", self.theme.header)),
            Cell::from(Span::styled("Display Name", self.theme.header)),
            Cell::from(Span::styled("Mail", self.theme.header)),
        ]);

        let rows: Vec<Row> = self
            .preview_results
            .iter()
            .take(max_rows)
            .map(|entry| {
                Row::new(vec![
                    Cell::from(Span::styled(&entry.dn, self.theme.normal)),
                    Cell::from(Span::styled(
                        entry.first_value("sAMAccountName").unwrap_or(""),
                        self.theme.normal,
                    )),
                    Cell::from(Span::styled(
                        entry.first_value("displayName").unwrap_or(""),
                        self.theme.normal,
                    )),
                    Cell::from(Span::styled(
                        entry.first_value("mail").unwrap_or(""),
                        self.theme.normal,
                    )),
                ])
            })
            .collect();

        let widths = [
            Constraint::Percentage(40),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ];

        let table = Table::new(rows, widths).header(header.style(self.theme.header));

        frame.render_widget(table, inner);
    }
}

impl CommandPanel {
    /// Render just the input field and completions popup (no messages, no border).
    /// Used inside the search popup.
    pub fn render_input_only(&self, frame: &mut Frame, area: Rect) {
        // Calculate input height: multi-line for compound filters, 1 otherwise
        let (formatted_lines, cursor_row, cursor_col, _) = if self.input_active {
            self.format_input_for_display()
        } else {
            (
                vec![self.input_buffer.clone()],
                0,
                self.input_buffer.len(),
                Vec::new(),
            )
        };

        if self.input_active {
            if formatted_lines.len() <= 1 {
                // Single-line rendering
                let before_cursor = &self.input_buffer[..self.cursor_pos];
                let at_cursor = if self.cursor_pos < self.input_buffer.len() {
                    &self.input_buffer[self.cursor_pos..self.cursor_pos + 1]
                } else {
                    "_"
                };
                let after_cursor = if self.cursor_pos < self.input_buffer.len() {
                    &self.input_buffer[self.cursor_pos + 1..]
                } else {
                    ""
                };

                let mut spans = vec![
                    Span::styled("/ ", self.theme.command_prompt),
                    Span::styled(before_cursor.to_string(), self.theme.normal),
                    Span::styled(at_cursor.to_string(), self.theme.command_prompt),
                ];
                if !after_cursor.is_empty() {
                    spans.push(Span::styled(after_cursor.to_string(), self.theme.normal));
                }
                if self.live_searching {
                    spans.push(Span::styled(" ...", self.theme.dimmed));
                }
                let input_line = Line::from(spans);
                frame.render_widget(Paragraph::new(input_line), area);
            } else {
                // Multi-line rendering for compound filters
                let display_lines: Vec<Line> = formatted_lines
                    .iter()
                    .enumerate()
                    .map(|(row_idx, line_text)| {
                        let prefix = if row_idx == 0 { "/ " } else { "  " };

                        if row_idx == cursor_row {
                            let before = &line_text[..cursor_col.min(line_text.len())];
                            let at = if cursor_col < line_text.len() {
                                &line_text[cursor_col..cursor_col + 1]
                            } else {
                                "_"
                            };
                            let after = if cursor_col < line_text.len() {
                                &line_text[cursor_col + 1..]
                            } else {
                                ""
                            };

                            let mut spans = vec![
                                Span::styled(prefix, self.theme.command_prompt),
                                Span::styled(before.to_string(), self.theme.normal),
                                Span::styled(at.to_string(), self.theme.command_prompt),
                            ];
                            if !after.is_empty() {
                                spans.push(Span::styled(after.to_string(), self.theme.normal));
                            }
                            Line::from(spans)
                        } else {
                            Line::from(vec![
                                Span::styled(prefix, self.theme.command_prompt),
                                Span::styled(line_text.clone(), self.theme.normal),
                            ])
                        }
                    })
                    .collect();
                frame.render_widget(Paragraph::new(display_lines), area);
            }
        } else {
            // Not active — show filter text as dimmed, or hint
            if self.input_buffer.is_empty() {
                let hint = Line::from(Span::styled("Press / to edit filter", self.theme.dimmed));
                frame.render_widget(Paragraph::new(hint), area);
            } else {
                let line = Line::from(vec![
                    Span::styled("/ ", self.theme.command_prompt),
                    Span::styled(&self.input_buffer, self.theme.dimmed),
                ]);
                frame.render_widget(Paragraph::new(line), area);
            }
        }

        // Render autocomplete popup above the input area
        if self.input_active {
            self.render_completion_popup(frame, area);
        }
    }
}

impl Component for CommandPanel {
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let border_style = if focused {
            self.theme.border_focused
        } else {
            self.theme.border
        };

        let mut block = Block::default()
            .title(" Command ")
            .borders(Borders::ALL)
            .border_style(border_style);
        if focused {
            block = block.border_type(BorderType::Double);
        }

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Calculate input height: multi-line for compound filters, 1 otherwise
        let (formatted_lines, cursor_row, cursor_col, _) = if self.input_active {
            self.format_input_for_display()
        } else {
            (vec![String::new()], 0, 0, Vec::new())
        };
        let input_height = (formatted_lines.len() as u16).clamp(1, 8);

        // Layout: messages (flex) | input area (dynamic height)
        let layout =
            Layout::vertical([Constraint::Min(1), Constraint::Length(input_height)]).split(inner);

        // Messages
        let msg_height = layout[0].height as usize;
        let start = self.messages.len().saturating_sub(msg_height);
        let visible_messages = &self.messages[start..];

        let lines: Vec<Line> = visible_messages
            .iter()
            .map(|msg| {
                let style = if msg.is_error {
                    self.theme.error
                } else {
                    self.theme.normal
                };
                Line::from(Span::styled(&msg.text, style))
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, layout[0]);

        // Input area with cursor position support
        if self.input_active {
            if formatted_lines.len() <= 1 {
                // Single-line rendering (original behavior)
                let before_cursor = &self.input_buffer[..self.cursor_pos];
                let at_cursor = if self.cursor_pos < self.input_buffer.len() {
                    &self.input_buffer[self.cursor_pos..self.cursor_pos + 1]
                } else {
                    "_"
                };
                let after_cursor = if self.cursor_pos < self.input_buffer.len() {
                    &self.input_buffer[self.cursor_pos + 1..]
                } else {
                    ""
                };

                let mut spans = vec![
                    Span::styled("/ ", self.theme.command_prompt),
                    Span::styled(before_cursor.to_string(), self.theme.normal),
                    Span::styled(at_cursor.to_string(), self.theme.command_prompt),
                ];
                if !after_cursor.is_empty() {
                    spans.push(Span::styled(after_cursor.to_string(), self.theme.normal));
                }
                if self.live_searching {
                    spans.push(Span::styled(" ...", self.theme.dimmed));
                }
                let input_line = Line::from(spans);
                frame.render_widget(Paragraph::new(input_line), layout[1]);
            } else {
                // Multi-line rendering for compound filters
                let display_lines: Vec<Line> = formatted_lines
                    .iter()
                    .enumerate()
                    .map(|(row_idx, line_text)| {
                        // First line gets "/ " prompt; others get "  " for alignment
                        let prefix = if row_idx == 0 { "/ " } else { "  " };

                        if row_idx == cursor_row {
                            // This line contains the cursor — split at cursor position
                            let before = &line_text[..cursor_col.min(line_text.len())];
                            let at = if cursor_col < line_text.len() {
                                &line_text[cursor_col..cursor_col + 1]
                            } else {
                                "_"
                            };
                            let after = if cursor_col < line_text.len() {
                                &line_text[cursor_col + 1..]
                            } else {
                                ""
                            };

                            let mut spans = vec![
                                Span::styled(prefix, self.theme.command_prompt),
                                Span::styled(before.to_string(), self.theme.normal),
                                Span::styled(at.to_string(), self.theme.command_prompt),
                            ];
                            if !after.is_empty() {
                                spans.push(Span::styled(after.to_string(), self.theme.normal));
                            }
                            Line::from(spans)
                        } else {
                            Line::from(vec![
                                Span::styled(prefix, self.theme.command_prompt),
                                Span::styled(line_text.clone(), self.theme.normal),
                            ])
                        }
                    })
                    .collect();
                frame.render_widget(Paragraph::new(display_lines), layout[1]);
            }
        } else if focused {
            let input_line = Line::from(Span::styled("Press / to search", self.theme.dimmed));
            frame.render_widget(Paragraph::new(input_line), layout[1]);
        }

        // Render inline preview of live search results above the panel
        if self.input_active {
            self.render_preview_overlay(frame, area);
        }

        // Render autocomplete popup above the panel (on top of preview)
        if self.input_active {
            self.render_completion_popup(frame, area);
        }
    }

    fn last_area(&self) -> Option<Rect> {
        self.area
    }
}
