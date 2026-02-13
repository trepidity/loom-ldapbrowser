use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// A clickable breadcrumb trail for DN navigation.
///
/// Displays a DN like "dc=com > dc=example > ou=Users > cn=Alice"
/// with each component as a distinct span.
pub struct Breadcrumb {
    parts: Vec<String>,
    separator_style: Style,
    part_style: Style,
    last_style: Style,
}

impl Breadcrumb {
    pub fn new(dn: &str, separator_style: Style, part_style: Style, last_style: Style) -> Self {
        // Split DN into RDN components and reverse for left-to-right reading
        let parts: Vec<String> = dn.split(',').map(|s| s.trim().to_string()).rev().collect();

        Self {
            parts,
            separator_style,
            part_style,
            last_style,
        }
    }

    /// Build the breadcrumb as a Line of styled spans.
    pub fn to_line(&self) -> Line<'_> {
        let mut spans = Vec::new();

        for (i, part) in self.parts.iter().enumerate() {
            let is_last = i == self.parts.len() - 1;
            let style = if is_last {
                self.last_style
            } else {
                self.part_style
            };

            spans.push(Span::styled(part.as_str(), style));

            if !is_last {
                spans.push(Span::styled(" > ", self.separator_style));
            }
        }

        Line::from(spans)
    }

    /// Get the full DN for the component at the given breadcrumb index.
    /// Returns the DN from this component down to the root.
    pub fn dn_at_index(&self, index: usize) -> Option<String> {
        if index >= self.parts.len() {
            return None;
        }

        // Reverse back to DN order (rightmost = root)
        let components: Vec<&str> = self.parts[..=index]
            .iter()
            .rev()
            .map(|s| s.as_str())
            .collect();

        Some(components.join(","))
    }

    /// Render the breadcrumb into the given area.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let line = self.to_line();
        let paragraph = Paragraph::new(line);
        frame.render_widget(paragraph, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breadcrumb_dn_at_index() {
        let bc = Breadcrumb::new(
            "cn=Alice,ou=Users,dc=example,dc=com",
            Style::default(),
            Style::default(),
            Style::default(),
        );

        // Reversed: ["dc=com", "dc=example", "ou=Users", "cn=Alice"]
        assert_eq!(bc.dn_at_index(0), Some("dc=com".to_string()));
        assert_eq!(bc.dn_at_index(1), Some("dc=example,dc=com".to_string()));
        assert_eq!(
            bc.dn_at_index(3),
            Some("cn=Alice,ou=Users,dc=example,dc=com".to_string())
        );
        assert_eq!(bc.dn_at_index(4), None);
    }
}
