use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::action::Action;
use crate::config::ConnectionProfile;
use crate::theme::Theme;

/// Dialog for selecting a connection profile to connect to.
pub struct ConnectDialog {
    pub visible: bool,
    profiles: Vec<ConnectionProfile>,
    list_state: ListState,
    theme: Theme,
}

impl ConnectDialog {
    pub fn new(theme: Theme) -> Self {
        Self {
            visible: false,
            profiles: Vec::new(),
            list_state: ListState::default(),
            theme,
        }
    }

    pub fn show(&mut self, profiles: Vec<ConnectionProfile>) {
        self.profiles = profiles;
        self.list_state.select(Some(0));
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Action {
        if !self.visible {
            return Action::None;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.list_state.selected().unwrap_or(0);
                if i > 0 {
                    self.list_state.select(Some(i - 1));
                }
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.list_state.selected().unwrap_or(0);
                if i + 1 < self.profiles.len() {
                    self.list_state.select(Some(i + 1));
                }
                Action::None
            }
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    self.visible = false;
                    Action::ConnectByIndex(idx)
                } else {
                    Action::None
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.hide();
                Action::ClosePopup
            }
            _ => Action::None,
        }
    }

    pub fn render(&self, frame: &mut Frame, full: Rect) {
        if !self.visible {
            return;
        }

        let popup_width = (full.width as u32 * 60 / 100).min(70) as u16;
        let popup_height = (self.profiles.len() as u16 + 4).min(full.height).max(6);

        let x = full.x + (full.width.saturating_sub(popup_width)) / 2;
        let y = full.y + (full.height.saturating_sub(popup_height)) / 2;
        let area = Rect::new(x, y, popup_width, popup_height);

        frame.render_widget(Clear, area);

        let block = Block::default()
            .title(" Connect ")
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border)
            .title_style(self.theme.popup_title);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.profiles.is_empty() {
            let msg = Paragraph::new(
                "No connection profiles configured.\nAdd profiles to ~/.config/loom/config.toml",
            )
            .style(self.theme.dimmed);
            frame.render_widget(msg, inner);
            return;
        }

        // Layout: hint (1 line) | list
        let layout = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(inner);

        let hint = Line::from(vec![
            Span::styled("  j/k", self.theme.header),
            Span::styled(": navigate  ", self.theme.dimmed),
            Span::styled("Enter", self.theme.header),
            Span::styled(": connect  ", self.theme.dimmed),
            Span::styled("Esc", self.theme.header),
            Span::styled(": cancel", self.theme.dimmed),
        ]);
        frame.render_widget(Paragraph::new(hint), layout[0]);

        let items: Vec<ListItem> = self
            .profiles
            .iter()
            .map(|p| {
                let line = Line::from(vec![
                    Span::styled(&p.name, self.theme.header),
                    Span::styled(format!("  {}:{}", p.host, p.port), self.theme.dimmed),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list =
            List::new(items).highlight_style(self.theme.selected.add_modifier(Modifier::BOLD));

        frame.render_stateful_widget(list, layout[1], &mut self.list_state.clone());
    }
}
