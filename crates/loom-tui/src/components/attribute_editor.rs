use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::action::Action;
use crate::theme::Theme;

/// Edit mode for an attribute value.
#[derive(Debug, Clone)]
pub enum EditOp {
    Replace { attr: String, old_value: String },
    Add { attr: String },
    Delete { attr: String, value: String },
}

/// Result of a completed edit operation.
#[derive(Debug, Clone)]
pub struct EditResult {
    pub dn: String,
    pub op: EditOp,
    pub new_value: String,
}

/// A popup dialog for editing a single attribute value.
pub struct AttributeEditor {
    pub visible: bool,
    dn: String,
    op: Option<EditOp>,
    input_buffer: String,
    cursor_pos: usize,
    theme: Theme,
}

impl AttributeEditor {
    pub fn new(theme: Theme) -> Self {
        Self {
            visible: false,
            dn: String::new(),
            op: None,
            input_buffer: String::new(),
            cursor_pos: 0,
            theme,
        }
    }

    /// Open editor to replace an existing attribute value.
    pub fn edit_value(&mut self, dn: String, attr: String, current_value: String) {
        self.dn = dn;
        self.input_buffer = current_value.clone();
        self.cursor_pos = self.input_buffer.len();
        self.op = Some(EditOp::Replace {
            attr,
            old_value: current_value,
        });
        self.visible = true;
    }

    /// Open editor to add a new value to an attribute.
    pub fn add_value(&mut self, dn: String, attr: String) {
        self.dn = dn;
        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.op = Some(EditOp::Add { attr });
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.op = None;
    }

    /// Get the label describing the current operation.
    fn op_label(&self) -> String {
        match &self.op {
            Some(EditOp::Replace { attr, .. }) => format!("Edit: {}", attr),
            Some(EditOp::Add { attr }) => format!("Add value to: {}", attr),
            Some(EditOp::Delete { attr, .. }) => format!("Delete from: {}", attr),
            None => "Edit".to_string(),
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Action {
        if !self.visible {
            return Action::None;
        }

        match key.code {
            KeyCode::Enter => {
                if let Some(op) = self.op.take() {
                    let result = EditResult {
                        dn: self.dn.clone(),
                        op,
                        new_value: self.input_buffer.clone(),
                    };
                    self.visible = false;
                    return Action::SaveAttribute(result);
                }
                Action::None
            }
            KeyCode::Esc => {
                self.hide();
                Action::ClosePopup
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.input_buffer.remove(self.cursor_pos);
                }
                Action::None
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_pos);
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
            KeyCode::Home => {
                self.cursor_pos = 0;
                Action::None
            }
            KeyCode::End => {
                self.cursor_pos = self.input_buffer.len();
                Action::None
            }
            KeyCode::Char(c) => {
                self.input_buffer.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
                Action::None
            }
            _ => Action::None,
        }
    }

    pub fn render(&self, frame: &mut Frame, full: Rect) {
        if !self.visible {
            return;
        }

        let popup_width = (full.width as u32 * 60 / 100).min(80) as u16;
        let popup_height = 7u16.min(full.height);

        let x = full.x + (full.width.saturating_sub(popup_width)) / 2;
        let y = full.y + (full.height.saturating_sub(popup_height)) / 2;
        let area = Rect::new(x, y, popup_width, popup_height);

        frame.render_widget(Clear, area);

        let title = format!(" {} ", self.op_label());
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border)
            .title_style(self.theme.popup_title);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

        // DN line
        let dn_line = Line::from(vec![
            Span::styled("DN: ", self.theme.header),
            Span::styled(&self.dn, self.theme.dimmed),
        ]);
        frame.render_widget(Paragraph::new(dn_line), layout[0]);

        // Input with cursor
        let (before_cursor, after_cursor) = self.input_buffer.split_at(self.cursor_pos);
        let input_line = Line::from(vec![
            Span::styled(before_cursor, self.theme.normal),
            Span::styled(
                if after_cursor.is_empty() {
                    "_"
                } else {
                    &after_cursor[..1]
                },
                self.theme.selected,
            ),
            Span::styled(
                if after_cursor.len() > 1 {
                    &after_cursor[1..]
                } else {
                    ""
                },
                self.theme.normal,
            ),
        ]);
        frame.render_widget(Paragraph::new(input_line), layout[1]);

        // Hint
        let hint = Line::from(Span::styled("Enter: save  Esc: cancel", self.theme.dimmed));
        frame.render_widget(Paragraph::new(hint), layout[2]);
    }
}
