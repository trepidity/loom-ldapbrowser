use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use loom_core::tls::CertificateInfo;

use crate::action::Action;
use crate::config::ConnectionProfile;
use crate::theme::Theme;

/// A dialog that shows untrusted certificate details and lets the user
/// choose to trust it always, for this session only, or reject it.
pub struct CertTrustDialog {
    pub visible: bool,
    cert_info: Option<CertificateInfo>,
    profile: Option<ConnectionProfile>,
    password: String,
    selected: usize, // 0=Always, 1=Session, 2=Reject
    theme: Theme,
}

impl CertTrustDialog {
    pub fn new(theme: Theme) -> Self {
        Self {
            visible: false,
            cert_info: None,
            profile: None,
            password: String::new(),
            selected: 2, // Default to Reject for safety
            theme,
        }
    }

    pub fn show(
        &mut self,
        cert_info: CertificateInfo,
        profile: ConnectionProfile,
        password: String,
    ) {
        self.cert_info = Some(cert_info);
        self.profile = Some(profile);
        self.password = password;
        self.selected = 2; // Default to Reject
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.cert_info = None;
        self.profile = None;
        self.password.clear();
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Action {
        if !self.visible {
            return Action::None;
        }

        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.selected < 2 {
                    self.selected += 1;
                }
                Action::None
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.accept(true)
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.accept(false)
            }
            KeyCode::Char('r') | KeyCode::Char('R') | KeyCode::Esc => {
                self.hide();
                Action::ClosePopup
            }
            KeyCode::Enter => match self.selected {
                0 => self.accept(true),
                1 => self.accept(false),
                _ => {
                    self.hide();
                    Action::ClosePopup
                }
            },
            _ => Action::None,
        }
    }

    fn accept(&mut self, always: bool) -> Action {
        let (cert_info, profile, password) = match (
            self.cert_info.take(),
            self.profile.take(),
            std::mem::take(&mut self.password),
        ) {
            (Some(ci), Some(p), pw) => (ci, p, pw),
            _ => {
                self.hide();
                return Action::ClosePopup;
            }
        };

        let fingerprint = cert_info.fingerprint_sha256.clone();
        self.visible = false;

        Action::TrustCertAndConnect {
            cert_info: Box::new(cert_info),
            fingerprint,
            always,
            profile: Box::new(profile),
            password,
        }
    }

    pub fn render(&self, frame: &mut Frame, full: Rect) {
        if !self.visible {
            return;
        }

        let info = match &self.cert_info {
            Some(info) => info,
            None => return,
        };

        // Size the popup
        let popup_width = 56u16.min(full.width.saturating_sub(4));
        let popup_height = 16u16.min(full.height.saturating_sub(2));

        let x = full.x + (full.width.saturating_sub(popup_width)) / 2;
        let y = full.y + (full.height.saturating_sub(popup_height)) / 2;
        let area = Rect::new(x, y, popup_width, popup_height);

        frame.render_widget(Clear, area);

        let block = Block::default()
            .title(" Untrusted Certificate ")
            .borders(Borders::ALL)
            .border_style(self.theme.warning)
            .title_style(self.theme.warning);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Layout: message + cert details (flex) | buttons (1 line)
        let layout =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

        // Build cert detail lines
        let host_port = format!("{}:{}", info.host, info.port);
        let fingerprint = &info.fingerprint_sha256;

        // Split fingerprint into two lines if it's long
        let (fp_line1, fp_line2) = if fingerprint.len() > 32 {
            let mid = fingerprint[..32]
                .rfind(':')
                .map(|i| i + 1)
                .unwrap_or(32);
            (&fingerprint[..mid], Some(&fingerprint[mid..]))
        } else {
            (fingerprint.as_str(), None)
        };

        let mut lines = vec![
            Line::from(Span::styled(
                "The server presented a certificate that is",
                self.theme.normal,
            )),
            Line::from(Span::styled(
                "not trusted by your system's certificate store.",
                self.theme.normal,
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Host:    ", self.theme.dimmed),
                Span::styled(host_port, self.theme.normal),
            ]),
            Line::from(vec![
                Span::styled("  Subject: ", self.theme.dimmed),
                Span::styled(&info.subject, self.theme.normal),
            ]),
            Line::from(vec![
                Span::styled("  Issuer:  ", self.theme.dimmed),
                Span::styled(&info.issuer, self.theme.normal),
            ]),
            Line::from(vec![
                Span::styled("  Valid:   ", self.theme.dimmed),
                Span::styled(
                    format!("{} to {}", info.not_before, info.not_after),
                    self.theme.normal,
                ),
            ]),
            Line::from(vec![
                Span::styled("  SHA-256: ", self.theme.dimmed),
                Span::styled(fp_line1, self.theme.normal),
            ]),
        ];

        if let Some(fp2) = fp_line2 {
            lines.push(Line::from(vec![
                Span::styled("           ", self.theme.dimmed),
                Span::styled(fp2, self.theme.normal),
            ]));
        }

        lines.push(Line::from(""));

        let msg = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(msg, layout[0]);

        // Buttons
        let always_style = if self.selected == 0 {
            self.theme.selected
        } else {
            self.theme.normal
        };
        let session_style = if self.selected == 1 {
            self.theme.selected
        } else {
            self.theme.normal
        };
        let reject_style = if self.selected == 2 {
            self.theme.selected
        } else {
            self.theme.normal
        };

        let buttons = Line::from(vec![
            Span::raw(" "),
            Span::styled(" [A]lways trust ", always_style),
            Span::raw(" "),
            Span::styled(" [S]ession only ", session_style),
            Span::raw(" "),
            Span::styled(" [R]eject ", reject_style),
        ]);

        frame.render_widget(Paragraph::new(buttons), layout[1]);
    }
}
