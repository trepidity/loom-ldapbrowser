use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::action::Action;
use crate::components::popup::Popup;
use crate::theme::Theme;

/// Export format options.
const FORMATS: &[(&str, &str)] = &[
    ("LDIF", ".ldif"),
    ("JSON", ".json"),
    ("CSV", ".csv"),
    ("Excel", ".xlsx"),
];

/// Dialog for exporting entries to a file.
pub struct ExportDialog {
    pub visible: bool,
    popup: Popup,
    theme: Theme,
    selected: usize,
    filename: String,
    editing_filename: bool,
    entry_count: usize,
}

impl ExportDialog {
    pub fn new(theme: Theme) -> Self {
        Self {
            visible: false,
            popup: Popup::new("Export Entries", theme.clone()).with_size(50, 40),
            theme,
            selected: 0,
            filename: String::new(),
            editing_filename: false,
            entry_count: 0,
        }
    }

    pub fn show(&mut self, entry_count: usize) {
        self.entry_count = entry_count;
        self.selected = 0;
        self.filename = format!("export{}", FORMATS[0].1);
        self.editing_filename = false;
        self.visible = true;
        self.popup.show();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.popup.hide();
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Action {
        if self.editing_filename {
            return self.handle_filename_key(key);
        }

        match key.code {
            KeyCode::Esc => {
                self.hide();
                Action::ClosePopup
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                    self.update_filename_ext();
                }
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < FORMATS.len() {
                    self.selected += 1;
                    self.update_filename_ext();
                }
                Action::None
            }
            KeyCode::Char('f') => {
                self.editing_filename = true;
                Action::None
            }
            KeyCode::Enter => {
                let path = self.filename.clone();
                self.hide();
                Action::ExportExecute(path)
            }
            _ => Action::None,
        }
    }

    fn handle_filename_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.editing_filename = false;
                Action::None
            }
            KeyCode::Backspace => {
                self.filename.pop();
                Action::None
            }
            KeyCode::Char(c) => {
                self.filename.push(c);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn update_filename_ext(&mut self) {
        // Replace extension based on selected format
        let ext = FORMATS[self.selected].1;
        if let Some(dot_pos) = self.filename.rfind('.') {
            self.filename.truncate(dot_pos);
        }
        self.filename.push_str(ext);
    }

    pub fn render(&self, frame: &mut Frame, full: Rect) {
        if !self.visible {
            return;
        }

        let area = self.popup.centered_area(full);
        frame.render_widget(Clear, area);

        let block = Block::default()
            .title(" Export Entries ")
            .borders(Borders::ALL)
            .border_style(self.theme.popup_border)
            .title_style(self.theme.popup_title);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Layout: info (2) | format list (formats + 1) | filename (2) | hints (1)
        let layout = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(FORMATS.len() as u16 + 1),
            Constraint::Length(2),
            Constraint::Min(1),
        ])
        .split(inner);

        // Info
        let info = Paragraph::new(vec![
            Line::from(Span::styled(
                format!("Entries to export: {}", self.entry_count),
                self.theme.normal,
            )),
            Line::from(Span::styled(
                "Select format and press Enter to export",
                self.theme.dimmed,
            )),
        ]);
        frame.render_widget(info, layout[0]);

        // Format list
        let mut format_lines = vec![Line::from(Span::styled("Format:", self.theme.header))];
        for (i, (name, ext)) in FORMATS.iter().enumerate() {
            let marker = if i == self.selected { "> " } else { "  " };
            let style = if i == self.selected {
                self.theme.selected.add_modifier(Modifier::BOLD)
            } else {
                self.theme.normal
            };
            format_lines.push(Line::from(Span::styled(
                format!("{}{} ({})", marker, name, ext),
                style,
            )));
        }
        frame.render_widget(Paragraph::new(format_lines), layout[1]);

        // Filename
        let filename_style = if self.editing_filename {
            self.theme.normal
        } else {
            self.theme.dimmed
        };
        let filename_lines = vec![
            Line::from(Span::styled("Filename:", self.theme.header)),
            Line::from(vec![
                Span::styled(&self.filename, filename_style),
                if self.editing_filename {
                    Span::styled("_", self.theme.command_prompt)
                } else {
                    Span::raw("")
                },
            ]),
        ];
        frame.render_widget(Paragraph::new(filename_lines), layout[2]);

        // Hints
        let hints = Paragraph::new(Line::from(Span::styled(
            "j/k:format  f:edit filename  Enter:export  Esc:cancel",
            self.theme.dimmed,
        )));
        frame.render_widget(hints, layout[3]);
    }
}
