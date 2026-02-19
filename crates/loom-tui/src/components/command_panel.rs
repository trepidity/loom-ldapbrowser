use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::action::Action;
use crate::component::Component;
use crate::theme::Theme;
use crate::widgets::fuzzy_input::{FuzzyFilter, FuzzyMatch};

/// The bottom-right panel: command input and status messages.
pub struct CommandPanel {
    pub messages: Vec<StatusMessage>,
    /// When true, the input line is active and capturing keystrokes.
    pub input_active: bool,
    pub input_buffer: String,
    theme: Theme,
    area: Option<Rect>,

    // Autocomplete state
    attribute_names: Vec<String>,
    fuzzy: FuzzyFilter,
    completions: Vec<FuzzyMatch>,
    completion_visible: bool,
    completion_selected: usize,
}

pub struct StatusMessage {
    pub text: String,
    pub is_error: bool,
}

impl CommandPanel {
    pub fn new(theme: Theme) -> Self {
        Self {
            messages: Vec::new(),
            input_active: false,
            input_buffer: String::new(),
            theme,
            area: None,
            attribute_names: Vec::new(),
            fuzzy: FuzzyFilter::new(),
            completions: Vec::new(),
            completion_visible: false,
            completion_selected: 0,
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
        self.hide_completions();
    }

    pub fn deactivate_input(&mut self) {
        self.input_active = false;
        self.input_buffer.clear();
        self.hide_completions();
    }

    /// Set the attribute names available for autocomplete.
    pub fn set_attribute_names(&mut self, names: Vec<String>) {
        self.attribute_names = names;
    }

    fn hide_completions(&mut self) {
        self.completion_visible = false;
        self.completions.clear();
        self.completion_selected = 0;
    }

    fn update_completions(&mut self) {
        if self.attribute_names.is_empty() {
            self.hide_completions();
            return;
        }

        match loom_core::filter::detect_attribute_context(&self.input_buffer) {
            Some(partial) => {
                self.completions = self.fuzzy.filter(&partial, &self.attribute_names);
                // Limit to a reasonable number
                self.completions.truncate(50);
                self.completion_visible = !self.completions.is_empty();
                // Clamp selection
                if self.completion_selected >= self.completions.len() {
                    self.completion_selected = 0;
                }
            }
            None => {
                self.hide_completions();
            }
        }
    }

    fn accept_completion(&mut self) {
        if !self.completion_visible || self.completions.is_empty() {
            return;
        }

        let selected = &self.completions[self.completion_selected];
        let attr_name = self.attribute_names[selected.index].clone();

        // Find the partial text we need to replace
        if let Some(partial) = loom_core::filter::detect_attribute_context(&self.input_buffer) {
            // Remove the partial from end of buffer
            let partial_len = partial.len();
            let new_len = self.input_buffer.len() - partial_len;
            self.input_buffer.truncate(new_len);
            // Append the full attribute name + '='
            self.input_buffer.push_str(&attr_name);
            self.input_buffer.push('=');
        }

        self.hide_completions();
    }

    /// Handle key events when the command panel is focused.
    /// Returns an Action for the app to dispatch.
    pub fn handle_input_key(&mut self, key: KeyEvent) -> Action {
        if !self.input_active {
            // Activate on '/' or ':'
            match key.code {
                KeyCode::Char('/') | KeyCode::Char(':') => {
                    self.activate_input();
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
                let query = self.input_buffer.clone();
                self.deactivate_input();
                if query.is_empty() {
                    Action::None
                } else {
                    self.push_message(format!("Search: {}", query));
                    Action::SearchExecute(query)
                }
            }
            KeyCode::Esc => {
                self.deactivate_input();
                Action::None
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
                self.update_completions();
                Action::None
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
                self.update_completions();
                Action::None
            }
            _ => Action::None,
        }
    }
}

impl CommandPanel {
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
        let title = if total > max_visible {
            format!(" Attributes ({}/{}) ", visible_count, total)
        } else {
            format!(" Attributes ({}) ", total)
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

        let items: Vec<ListItem> = self.completions[scroll_offset..]
            .iter()
            .take(max_visible)
            .enumerate()
            .map(|(i, m)| {
                let name = &self.attribute_names[m.index];
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

        // Layout: messages (flex) | input line (1)
        let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

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

        // Input line
        let input_line = if self.input_active {
            Line::from(vec![
                Span::styled("/ ", self.theme.command_prompt),
                Span::styled(&self.input_buffer, self.theme.normal),
                Span::styled("_", self.theme.command_prompt),
            ])
        } else if focused {
            Line::from(Span::styled("Press / to search", self.theme.dimmed))
        } else {
            Line::from(Span::raw(""))
        };

        frame.render_widget(Paragraph::new(input_line), layout[1]);

        // Render autocomplete popup above the panel
        if self.input_active {
            self.render_completion_popup(frame, area);
        }
    }

    fn last_area(&self) -> Option<Rect> {
        self.area
    }
}
