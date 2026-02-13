use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::ConnectionId;
use crate::theme::Theme;

/// A single tab entry.
#[derive(Debug, Clone)]
pub struct TabEntry {
    pub id: ConnectionId,
    pub label: String,
}

/// The tab bar showing open connection tabs.
pub struct TabBar {
    pub tabs: Vec<TabEntry>,
    pub active_tab: Option<ConnectionId>,
    theme: Theme,
}

impl TabBar {
    pub fn new(theme: Theme) -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
            theme,
        }
    }

    pub fn add_tab(&mut self, id: ConnectionId, label: String) {
        self.tabs.push(TabEntry { id, label });
        self.active_tab = Some(id);
    }

    pub fn remove_tab(&mut self, id: ConnectionId) {
        self.tabs.retain(|t| t.id != id);
        if self.active_tab == Some(id) {
            self.active_tab = self.tabs.first().map(|t| t.id);
        }
    }

    pub fn set_active(&mut self, id: ConnectionId) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_tab = Some(id);
        }
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(active) = self.active_tab {
            let idx = self.tabs.iter().position(|t| t.id == active).unwrap_or(0);
            let next = (idx + 1) % self.tabs.len();
            self.active_tab = Some(self.tabs[next].id);
        }
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        if let Some(active) = self.active_tab {
            let idx = self.tabs.iter().position(|t| t.id == active).unwrap_or(0);
            let prev = (idx + self.tabs.len() - 1) % self.tabs.len();
            self.active_tab = Some(self.tabs[prev].id);
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.tabs.is_empty() {
            let line = Line::from(vec![
                Span::styled("  No connections  ", self.theme.dimmed),
                Span::styled("[+] New", self.theme.header),
            ]);
            let bar = Paragraph::new(line).style(self.theme.status_bar);
            frame.render_widget(bar, area);
            return;
        }

        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(" ", self.theme.status_bar));

        for tab in &self.tabs {
            let is_active = self.active_tab == Some(tab.id);
            let style = if is_active {
                self.theme.tab_active
            } else {
                self.theme.tab_inactive
            };

            if is_active {
                spans.push(Span::styled("[", style));
                spans.push(Span::styled(&tab.label, style));
                spans.push(Span::styled("]", style));
            } else {
                spans.push(Span::styled(" ", self.theme.status_bar));
                spans.push(Span::styled(&tab.label, style));
                spans.push(Span::styled(" ", self.theme.status_bar));
            }
            spans.push(Span::styled(" ", self.theme.status_bar));
        }

        spans.push(Span::styled("[+]", self.theme.header));

        // Pad remaining width
        let content_len: usize = spans.iter().map(|s| s.content.len()).sum();
        let padding = " ".repeat(area.width as usize - content_len.min(area.width as usize));
        spans.push(Span::styled(padding, self.theme.status_bar));

        let line = Line::from(spans);
        let bar = Paragraph::new(line);
        frame.render_widget(bar, area);
    }
}
