use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::{ActiveLayout, ConnectionId};
use crate::components::tab_bar::TabEntry;
use crate::theme::Theme;

/// Top-level bar combining layout toggle and connection tabs in a single row.
///
/// Browser mode:  ` [Browser]  Connections  │ [conn1] conn2 [+]`
/// Connections mode: ` Browser  [Connections]`
pub struct LayoutBar {
    pub active: ActiveLayout,
    theme: Theme,
}

impl LayoutBar {
    pub fn new(theme: Theme) -> Self {
        Self {
            active: ActiveLayout::Browser,
            theme,
        }
    }

    pub fn render(
        &self,
        frame: &mut Frame,
        area: Rect,
        tabs: &[TabEntry],
        active_tab: Option<ConnectionId>,
    ) {
        let browser_style = if self.active == ActiveLayout::Browser {
            self.theme.tab_active
        } else {
            self.theme.tab_inactive
        };
        let conns_style = if self.active == ActiveLayout::Connections {
            self.theme.tab_active
        } else {
            self.theme.tab_inactive
        };

        let mut spans = vec![Span::styled(" ", self.theme.status_bar)];

        if self.active == ActiveLayout::Browser {
            spans.push(Span::styled("[Browser]", browser_style));
        } else {
            spans.push(Span::styled(" Browser ", browser_style));
        }

        spans.push(Span::styled("  ", self.theme.status_bar));

        if self.active == ActiveLayout::Connections {
            spans.push(Span::styled("[Connections]", conns_style));
        } else {
            spans.push(Span::styled(" Connections ", conns_style));
        }

        // In Browser mode, append connection tabs after a separator
        if self.active == ActiveLayout::Browser {
            spans.push(Span::styled(" \u{2502} ", self.theme.dimmed)); // │ separator

            if tabs.is_empty() {
                spans.push(Span::styled("[+] New", self.theme.header));
            } else {
                for tab in tabs {
                    let is_active = active_tab == Some(tab.id);
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
            }
        }

        // Pad remaining width
        let content_len: usize = spans.iter().map(|s| s.content.len()).sum();
        let padding = " ".repeat(area.width as usize - content_len.min(area.width as usize));
        spans.push(Span::styled(padding, self.theme.status_bar));

        let line = Line::from(spans);
        let bar = Paragraph::new(line);
        frame.render_widget(bar, area);
    }
}
