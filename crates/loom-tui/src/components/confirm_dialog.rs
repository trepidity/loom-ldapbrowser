use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::action::Action;
use crate::theme::Theme;

/// A confirmation dialog: "Are you sure?" with Yes/No buttons.
pub struct ConfirmDialog {
    pub visible: bool,
    pub message: String,
    pub on_confirm: Option<Box<Action>>,
    selected: usize, // 0 = Yes, 1 = No
    theme: Theme,
}

impl ConfirmDialog {
    pub fn new(theme: Theme) -> Self {
        Self {
            visible: false,
            message: String::new(),
            on_confirm: None,
            selected: 1, // Default to No for safety
            theme,
        }
    }

    pub fn show(&mut self, message: String, on_confirm: Action) {
        self.message = message;
        self.on_confirm = Some(Box::new(on_confirm));
        self.selected = 1;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.on_confirm = None;
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Action {
        if !self.visible {
            return Action::None;
        }

        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.selected = 0;
                Action::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.selected = 1;
                Action::None
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.visible = false;
                self.on_confirm.take().map(|a| *a).unwrap_or(Action::None)
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.hide();
                Action::ClosePopup
            }
            KeyCode::Enter => {
                if self.selected == 0 {
                    self.visible = false;
                    self.on_confirm.take().map(|a| *a).unwrap_or(Action::None)
                } else {
                    self.hide();
                    Action::ClosePopup
                }
            }
            _ => Action::None,
        }
    }

    pub fn render(&self, frame: &mut Frame, full: Rect) {
        if !self.visible {
            return;
        }

        // Center a 50x10 popup
        let popup_width = (full.width as u32 * 50 / 100).min(60) as u16;
        let popup_height = 8u16.min(full.height);

        let x = full.x + (full.width.saturating_sub(popup_width)) / 2;
        let y = full.y + (full.height.saturating_sub(popup_height)) / 2;
        let area = Rect::new(x, y, popup_width, popup_height);

        frame.render_widget(Clear, area);

        let block = Block::default()
            .title(" Confirm ")
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border)
            .title_style(self.theme.popup_title);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Layout: message (flex) | buttons (1 line)
        let layout = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

        // Message
        let msg = Paragraph::new(self.message.as_str())
            .style(self.theme.normal)
            .wrap(Wrap { trim: true });
        frame.render_widget(msg, layout[0]);

        // Buttons
        let yes_style = if self.selected == 0 {
            self.theme.selected
        } else {
            self.theme.normal
        };
        let no_style = if self.selected == 1 {
            self.theme.selected
        } else {
            self.theme.normal
        };

        let buttons = Line::from(vec![
            Span::raw("  "),
            Span::styled(" [Y]es ", yes_style),
            Span::raw("   "),
            Span::styled(" [N]o ", no_style),
        ]);

        frame.render_widget(Paragraph::new(buttons), layout[1]);
    }
}
