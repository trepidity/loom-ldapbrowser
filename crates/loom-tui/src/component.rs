use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::action::Action;

/// Trait for all UI components (panels, dialogs, widgets).
pub trait Component {
    /// Handle a key event. Return an Action to dispatch.
    fn handle_key_event(&mut self, _key: KeyEvent) -> Action {
        Action::None
    }

    /// Handle a mouse event. Return an Action to dispatch.
    fn handle_mouse_event(&mut self, _mouse: MouseEvent) -> Action {
        Action::None
    }

    /// Update state in response to an action. May return a new action to chain.
    fn update(&mut self, _action: &Action) -> Action {
        Action::None
    }

    /// Render the component into the given area.
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);

    /// Return the area this component last rendered into (for mouse hit testing).
    fn last_area(&self) -> Option<Rect> {
        None
    }
}
