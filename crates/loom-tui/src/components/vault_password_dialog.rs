use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::action::Action;
use crate::components::popup::Popup;
use crate::theme::Theme;

/// What the vault password dialog is being used for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VaultDialogMode {
    /// Creating a new vault — asks for master password + confirmation.
    CreateVault,
    /// Storing a profile's LDAP password into an existing vault.
    StoreProfilePassword,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Field {
    Password,
    Confirm,
}

/// Dialog for entering a vault master password (create) or a profile password (store).
pub struct VaultPasswordDialog {
    pub visible: bool,
    popup: Popup,
    theme: Theme,
    mode: VaultDialogMode,
    active_field: Field,
    password: String,
    confirm: String,
    profile_name: Option<String>,
}

impl VaultPasswordDialog {
    pub fn new(theme: Theme) -> Self {
        Self {
            visible: false,
            popup: Popup::new("Vault Password", theme.clone()).with_size(60, 30),
            theme,
            mode: VaultDialogMode::CreateVault,
            active_field: Field::Password,
            password: String::new(),
            confirm: String::new(),
            profile_name: None,
        }
    }

    /// Show dialog for creating a new vault.
    pub fn show_create(&mut self) {
        self.mode = VaultDialogMode::CreateVault;
        self.password.clear();
        self.confirm.clear();
        self.active_field = Field::Password;
        self.profile_name = None;
        self.visible = true;
        self.popup.show();
    }

    /// Show dialog for storing a profile's LDAP password.
    pub fn show_store_password(&mut self, profile_name: &str) {
        self.mode = VaultDialogMode::StoreProfilePassword;
        self.password.clear();
        self.confirm.clear();
        self.active_field = Field::Password;
        self.profile_name = Some(profile_name.to_string());
        self.visible = true;
        self.popup.show();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.popup.hide();
        self.password.clear();
        self.confirm.clear();
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.hide();
                Action::ClosePopup
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if self.mode == VaultDialogMode::CreateVault {
                    self.active_field = match self.active_field {
                        Field::Password => Field::Confirm,
                        Field::Confirm => Field::Password,
                    };
                }
                Action::None
            }
            KeyCode::Enter => self.submit(),
            KeyCode::Backspace => {
                self.active_buffer_mut().pop();
                Action::None
            }
            KeyCode::Char(c) => {
                self.active_buffer_mut().push(c);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn submit(&mut self) -> Action {
        if self.password.is_empty() {
            return Action::ErrorMessage("Password cannot be empty".to_string());
        }

        match self.mode {
            VaultDialogMode::CreateVault => {
                if self.password != self.confirm {
                    return Action::ErrorMessage("Passwords do not match".to_string());
                }
                let password = self.password.clone();
                self.hide();
                Action::VaultPasswordEntered(password)
            }
            VaultDialogMode::StoreProfilePassword => {
                let profile_name = self.profile_name.clone().unwrap_or_default();
                let password = self.password.clone();
                self.hide();
                Action::VaultStorePassword(profile_name, password)
            }
        }
    }

    fn active_buffer_mut(&mut self) -> &mut String {
        match self.active_field {
            Field::Password => &mut self.password,
            Field::Confirm => &mut self.confirm,
        }
    }

    pub fn render(&self, frame: &mut Frame, full: Rect) {
        if !self.visible {
            return;
        }

        let area = self.popup.centered_area(full);
        frame.render_widget(Clear, area);

        let title = match self.mode {
            VaultDialogMode::CreateVault => " Create Vault ",
            VaultDialogMode::StoreProfilePassword => " Store Password in Vault ",
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border)
            .title_style(self.theme.popup_title);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        match self.mode {
            VaultDialogMode::CreateVault => self.render_create(frame, inner),
            VaultDialogMode::StoreProfilePassword => self.render_store(frame, inner),
        }
    }

    fn render_create(&self, frame: &mut Frame, inner: Rect) {
        let layout = Layout::vertical([
            Constraint::Length(2), // Info
            Constraint::Length(2), // Password
            Constraint::Length(2), // Confirm
            Constraint::Min(1),    // Hints
        ])
        .split(inner);

        let info = Paragraph::new(Line::from(Span::styled(
            "Choose a master password for the vault:",
            self.theme.normal,
        )));
        frame.render_widget(info, layout[0]);

        self.render_field(
            frame,
            layout[1],
            "Password",
            &self.password,
            Field::Password,
        );
        self.render_field(frame, layout[2], "Confirm", &self.confirm, Field::Confirm);

        let hints = Paragraph::new(Line::from(Span::styled(
            "Tab:switch field  Enter:create  Esc:cancel",
            self.theme.dimmed,
        )));
        frame.render_widget(hints, layout[3]);
    }

    fn render_store(&self, frame: &mut Frame, inner: Rect) {
        let layout = Layout::vertical([
            Constraint::Length(2), // Info
            Constraint::Length(2), // Password
            Constraint::Min(1),    // Hints
        ])
        .split(inner);

        let profile_label = self.profile_name.as_deref().unwrap_or("(unknown)");
        let info = Paragraph::new(Line::from(vec![
            Span::styled("Password for profile: ", self.theme.dimmed),
            Span::styled(profile_label, self.theme.normal),
        ]));
        frame.render_widget(info, layout[0]);

        self.render_field(
            frame,
            layout[1],
            "Password",
            &self.password,
            Field::Password,
        );

        let hints = Paragraph::new(Line::from(Span::styled(
            "Enter:store  Esc:cancel",
            self.theme.dimmed,
        )));
        frame.render_widget(hints, layout[2]);
    }

    fn render_field(&self, frame: &mut Frame, area: Rect, label: &str, value: &str, field: Field) {
        let is_active = self.active_field == field;
        let label_style = if is_active {
            self.theme.header
        } else {
            self.theme.dimmed
        };
        let value_style = if is_active {
            self.theme.normal
        } else {
            self.theme.dimmed
        };

        let display_value = if !value.is_empty() {
            "*".repeat(value.len())
        } else {
            String::new()
        };

        let lines = vec![
            Line::from(Span::styled(format!("{}:", label), label_style)),
            Line::from(vec![
                Span::styled(display_value, value_style),
                if is_active {
                    Span::styled("_", self.theme.command_prompt)
                } else {
                    Span::raw("")
                },
            ]),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }
}
