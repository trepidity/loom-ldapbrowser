use ratatui::style::Style;
use ratatui::text::Span;

/// Braille-based spinner for loading indicators.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// A simple animated spinner widget.
pub struct Spinner {
    tick: usize,
    style: Style,
}

impl Spinner {
    pub fn new(style: Style) -> Self {
        Self { tick: 0, style }
    }

    /// Advance the spinner one frame.
    pub fn tick(&mut self) {
        self.tick = (self.tick + 1) % SPINNER_FRAMES.len();
    }

    /// Get the current spinner frame as a styled Span.
    pub fn span(&self) -> Span<'_> {
        Span::styled(SPINNER_FRAMES[self.tick], self.style)
    }

    /// Get the current spinner frame as a string.
    pub fn frame(&self) -> &'static str {
        SPINNER_FRAMES[self.tick]
    }
}
