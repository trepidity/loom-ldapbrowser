use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::{ActiveLayout, ConnectionId};
use crate::components::tab_bar::TabEntry;
use crate::theme::Theme;

/// Unified tab bar: `[Profiles] | [conn1] conn2`
///
/// Profiles is always the first tab. Connection tabs follow after a separator.
pub struct LayoutBar {
    pub active: ActiveLayout,
    theme: Theme,
    /// Hit regions populated during render: (x_start, x_end_exclusive, target).
    /// `None` = Profiles tab, `Some(id)` = connection tab.
    pub hit_regions: Vec<(u16, u16, Option<ConnectionId>)>,
}

impl LayoutBar {
    pub fn new(theme: Theme) -> Self {
        Self {
            active: ActiveLayout::Profiles,
            theme,
            hit_regions: Vec::new(),
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        tabs: &[TabEntry],
        active_tab: Option<ConnectionId>,
    ) {
        self.hit_regions.clear();

        let profiles_active = self.active == ActiveLayout::Profiles;
        let profiles_style = if profiles_active {
            self.theme.tab_active
        } else {
            self.theme.tab_inactive
        };

        let mut spans = vec![Span::styled(" ", self.theme.status_bar)];
        let mut x = area.x + 1; // after leading space

        // Profiles tab
        let profiles_label = if profiles_active {
            "[Profiles]"
        } else {
            " Profiles "
        };
        spans.push(Span::styled(profiles_label, profiles_style));
        let profiles_end = x + profiles_label.len() as u16;
        self.hit_regions.push((x, profiles_end, None));
        x = profiles_end;

        // Connection tabs after separator
        if !tabs.is_empty() {
            spans.push(Span::styled(" \u{2502} ", self.theme.dimmed));
            x += 3;

            for tab in tabs {
                let is_active =
                    self.active == ActiveLayout::Browser && active_tab == Some(tab.id);
                let style = if is_active {
                    self.theme.tab_active
                } else {
                    self.theme.tab_inactive
                };

                let tab_start = x;
                if is_active {
                    spans.push(Span::styled("[", style));
                    spans.push(Span::styled(&tab.label, style));
                    spans.push(Span::styled("]", style));
                } else {
                    spans.push(Span::styled(" ", self.theme.status_bar));
                    spans.push(Span::styled(&tab.label, style));
                    spans.push(Span::styled(" ", self.theme.status_bar));
                }
                x += tab.label.len() as u16 + 2;
                self.hit_regions.push((tab_start, x, Some(tab.id)));

                spans.push(Span::styled(" ", self.theme.status_bar));
                x += 1;
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
