use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::{Action, FocusTarget};

/// Resolve a key event to an action based on the current focus context.
pub fn resolve_key(key: KeyEvent, focus: FocusTarget) -> Action {
    // Global keybindings (always active)
    if let Some(action) = resolve_global(key) {
        return action;
    }

    // Context-specific keybindings
    match focus {
        FocusTarget::TreePanel => resolve_tree(key),
        FocusTarget::DetailPanel => resolve_detail(key),
        FocusTarget::CommandPanel => resolve_command(key),
    }
}

fn resolve_global(key: KeyEvent) -> Option<Action> {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('q')) => Some(Action::Quit),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(Action::Quit),
        (KeyModifiers::NONE, KeyCode::Tab) => Some(Action::FocusNext),
        (KeyModifiers::SHIFT, KeyCode::BackTab) => Some(Action::FocusPrev),
        // Tab management
        (KeyModifiers::CONTROL, KeyCode::Char('t')) => Some(Action::ShowConnectDialog),
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Some(Action::NextTab),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Some(Action::PrevTab),
        // Search shortcut
        (KeyModifiers::NONE, KeyCode::Char('/')) => Some(Action::SearchFocusInput),
        // Export
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => Some(Action::ShowExportDialog),
        // Bulk update
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => Some(Action::ShowBulkUpdateDialog),
        // Schema viewer
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => Some(Action::ShowSchemaViewer),
        // Log panel
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => Some(Action::ToggleLogPanel),
        // Save current ad-hoc connection
        (KeyModifiers::CONTROL, KeyCode::Char('w')) => Some(Action::SaveCurrentConnection),
        _ => None,
    }
}

fn resolve_tree(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Action::TreeUp,
        KeyCode::Down | KeyCode::Char('j') => Action::TreeDown,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => Action::TreeToggle,
        KeyCode::Left | KeyCode::Char('h') => Action::TreeCollapse(String::new()),
        _ => Action::None,
    }
}

fn resolve_detail(key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('r') => Action::EntryRefresh,
        _ => Action::None,
    }
}

fn resolve_command(_key: KeyEvent) -> Action {
    // Command panel key handling is done inline by the component
    // since it needs to capture text input
    Action::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn test_quit() {
        let action = resolve_key(key(KeyCode::Char('q')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn test_ctrl_c_quit() {
        let action = resolve_key(ctrl(KeyCode::Char('c')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn test_tab_focus() {
        let action = resolve_key(key(KeyCode::Tab), FocusTarget::TreePanel);
        assert!(matches!(action, Action::FocusNext));
    }

    #[test]
    fn test_search_shortcut() {
        let action = resolve_key(key(KeyCode::Char('/')), FocusTarget::DetailPanel);
        assert!(matches!(action, Action::SearchFocusInput));
    }

    #[test]
    fn test_tree_navigation() {
        let action = resolve_key(key(KeyCode::Char('j')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::TreeDown));

        let action = resolve_key(key(KeyCode::Char('k')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::TreeUp));
    }

    #[test]
    fn test_ctrl_t_connect() {
        let action = resolve_key(ctrl(KeyCode::Char('t')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::ShowConnectDialog));
    }

    #[test]
    fn test_ctrl_e_export() {
        let action = resolve_key(ctrl(KeyCode::Char('e')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::ShowExportDialog));
    }

    #[test]
    fn test_ctrl_s_schema() {
        let action = resolve_key(ctrl(KeyCode::Char('s')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::ShowSchemaViewer));
    }

    #[test]
    fn test_ctrl_w_save_connection() {
        let action = resolve_key(ctrl(KeyCode::Char('w')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::SaveCurrentConnection));
    }

    #[test]
    fn test_tree_panel_a_no_action_in_keymap() {
        // 'a' in tree context falls through to Action::None in the global keymap
        // (the actual ShowCreateEntryDialog is handled by TreePanel::handle_key_event)
        let action = resolve_key(key(KeyCode::Char('a')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn test_tree_panel_d_no_action_in_keymap() {
        // 'd' in tree context falls through to Action::None in the global keymap
        // (the actual delete confirm is handled by TreePanel::handle_key_event)
        let action = resolve_key(key(KeyCode::Char('d')), FocusTarget::TreePanel);
        assert!(matches!(action, Action::None));
    }
}
