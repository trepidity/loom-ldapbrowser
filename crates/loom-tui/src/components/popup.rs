use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::theme::Theme;

/// A generic popup overlay. Components render their content into the inner area.
pub struct Popup {
    pub title: String,
    pub visible: bool,
    theme: Theme,
    width_percent: u16,
    height_percent: u16,
}

impl Popup {
    pub fn new(title: impl Into<String>, theme: Theme) -> Self {
        Self {
            title: title.into(),
            visible: false,
            theme,
            width_percent: 50,
            height_percent: 40,
        }
    }

    pub fn with_size(mut self, width_percent: u16, height_percent: u16) -> Self {
        self.width_percent = width_percent;
        self.height_percent = height_percent;
        self
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Calculate the centered popup area within the given full area.
    pub fn centered_area(&self, full: Rect) -> Rect {
        let popup_width = (full.width as u32 * self.width_percent as u32 / 100) as u16;
        let popup_height = (full.height as u32 * self.height_percent as u32 / 100) as u16;

        let horizontal = Layout::horizontal([Constraint::Length(popup_width)])
            .flex(Flex::Center)
            .split(full);

        let vertical = Layout::vertical([Constraint::Length(popup_height)])
            .flex(Flex::Center)
            .split(horizontal[0]);

        vertical[0]
    }

    /// Render the popup frame (border + title + clear background).
    /// Returns the inner area for the caller to render content into.
    pub fn render_frame(&self, frame: &mut Frame, full: Rect) -> Rect {
        let area = self.centered_area(full);

        // Clear the area behind the popup
        frame.render_widget(Clear, area);

        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border)
            .title_style(self.theme.popup_title);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        inner
    }
}

/// A simple message popup with just text content.
pub fn render_message_popup(
    frame: &mut Frame,
    full: Rect,
    title: &str,
    message: &str,
    theme: &Theme,
) {
    let popup = Popup::new(title, theme.clone()).with_size(50, 30);
    let inner = popup.render_frame(frame, full);

    let paragraph = Paragraph::new(message)
        .style(theme.normal)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, inner);
}
