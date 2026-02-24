/// The kind of filter context the cursor is in.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterContext {
    /// Cursor in attribute-name position. `partial` is text typed so far.
    AttributeName { partial: String },
    /// Cursor in value position after operator. `attr` is the attribute name.
    Value { attr: String, partial: String },
    /// Buffer is empty or just `(` — show template suggestions.
    Empty,
}

/// Detect the filter context at the cursor position (end of input).
///
/// Returns:
/// - `Some(FilterContext::Empty)` when input is empty or just `(`
/// - `Some(FilterContext::AttributeName)` when cursor is in attribute-name position
/// - `Some(FilterContext::Value)` when cursor is after an operator in value position
/// - `None` when all parens are matched (filter looks complete)
pub fn detect_filter_context(input: &str) -> Option<FilterContext> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "(" {
        return Some(FilterContext::Empty);
    }

    // Use a stack to track unmatched '(' positions
    let mut stack = Vec::new();
    for (i, ch) in input.char_indices() {
        match ch {
            '(' => stack.push(i),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }

    let open_pos = match stack.last() {
        Some(&pos) => pos,
        None => return None, // all parens matched
    };

    let after_paren = &input[open_pos + 1..];

    // Strip leading boolean operators (&, |, !)
    let content = after_paren.trim_start_matches(['&', '|', '!']);

    // Check for comparison operators to determine if we're in value position
    // Try two-char operators first, then single '='
    for op in &["~=", ">=", "<=", "="] {
        if let Some(op_pos) = content.find(op) {
            let attr = content[..op_pos].to_string();
            let value_start = op_pos + op.len();
            let partial = content[value_start..].to_string();
            return Some(FilterContext::Value { attr, partial });
        }
    }

    // No operator found — we're in attribute-name position
    Some(FilterContext::AttributeName {
        partial: content.to_string(),
    })
}

/// Detect whether the cursor (at end of input) is in attribute-name position.
/// Returns `Some(partial)` with the partial attribute text if so, `None` if
/// the user is in value position or the input doesn't look like a filter context.
pub fn detect_attribute_context(input: &str) -> Option<String> {
    // Use a stack to track unmatched '(' positions
    let mut stack = Vec::new();
    for (i, ch) in input.char_indices() {
        match ch {
            '(' => stack.push(i),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }

    let open_pos = *stack.last()?;
    let after_paren = &input[open_pos + 1..];

    // If there's a comparison operator, we're in value position
    if after_paren.contains("~=")
        || after_paren.contains(">=")
        || after_paren.contains("<=")
        || after_paren.contains('=')
    {
        return None;
    }

    // Strip leading boolean operators (&, |, !)
    let partial = after_paren.trim_start_matches(['&', '|', '!']);

    Some(partial.to_string())
}

/// Validate that a string is a valid LDAP search filter per RFC 4515.
///
/// ```text
/// filter     = "(" filtercomp ")"
/// filtercomp = and / or / not / item
/// and        = "&" filterlist
/// or         = "|" filterlist
/// not        = "!" filter
/// filterlist = 1*filter
/// item       = attr filtertype value
/// filtertype = "=" / "~=" / ">=" / "<="
/// ```
pub fn validate_filter(filter: &str) -> Result<(), String> {
    let filter = filter.trim();
    if filter.is_empty() {
        return Err("Filter cannot be empty".to_string());
    }

    let bytes = filter.as_bytes();
    let end = parse_filter(bytes, 0)?;
    if end != bytes.len() {
        Err(format!(
            "Unexpected characters after filter at position {}",
            end + 1
        ))
    } else {
        Ok(())
    }
}

/// Parse a single filter: "(" filtercomp ")"
/// Returns the position after the closing ')'.
fn parse_filter(input: &[u8], pos: usize) -> Result<usize, String> {
    if pos >= input.len() {
        return Err(format!("Expected '(' at position {}", pos + 1));
    }
    if input[pos] != b'(' {
        return Err(format!("Expected '(' at position {}", pos + 1));
    }

    let inner = pos + 1;
    if inner >= input.len() {
        return Err(format!(
            "Unexpected end of filter after '(' at position {}",
            pos + 1
        ));
    }

    let end = match input[inner] {
        b'&' => parse_filter_list(input, inner + 1, '&')?,
        b'|' => parse_filter_list(input, inner + 1, '|')?,
        b'!' => parse_filter(input, inner + 1)?,
        _ => parse_item(input, inner)?,
    };

    if end >= input.len() {
        return Err(format!("Expected ')' at position {}", end + 1));
    }
    if input[end] != b')' {
        return Err(format!("Expected ')' at position {}", end + 1));
    }
    Ok(end + 1)
}

/// Parse a filterlist: 1*filter
/// The operator char is only used for error messages.
fn parse_filter_list(input: &[u8], pos: usize, op: char) -> Result<usize, String> {
    if pos >= input.len() || input[pos] != b'(' {
        return Err(format!(
            "Empty filter list in '{}' operator at position {}",
            op,
            pos + 1
        ));
    }
    let mut cur = pos;
    let mut count = 0;
    while cur < input.len() && input[cur] == b'(' {
        cur = parse_filter(input, cur)?;
        count += 1;
    }
    if count == 0 {
        return Err(format!(
            "Empty filter list in '{}' operator at position {}",
            op,
            pos + 1
        ));
    }
    Ok(cur)
}

/// Parse a simple filter item: attr filtertype value
/// Returns position after the value (just before the closing ')').
fn parse_item(input: &[u8], pos: usize) -> Result<usize, String> {
    // Parse attribute name: alphanumeric, hyphen, period, semicolon (for options like ;binary)
    let attr_start = pos;
    let mut cur = pos;
    while cur < input.len()
        && (input[cur].is_ascii_alphanumeric()
            || input[cur] == b'-'
            || input[cur] == b'.'
            || input[cur] == b';')
    {
        cur += 1;
    }

    if cur == attr_start {
        return Err(format!(
            "Expected attribute name after '(' at position {}",
            pos + 1
        ));
    }

    if cur >= input.len() {
        return Err(
            "Expected comparison operator (=, ~=, >=, <=) after attribute name".to_string(),
        );
    }

    // Parse filtertype: =, ~=, >=, <=
    if cur + 1 < input.len() && input[cur + 1] == b'=' {
        match input[cur] {
            b'~' | b'>' | b'<' => {
                cur += 2;
            }
            b'=' => {
                // Just '=' — single char operator
                cur += 1;
            }
            _ => {
                return Err(
                    "Expected comparison operator (=, ~=, >=, <=) after attribute name".to_string(),
                );
            }
        }
    } else if cur < input.len() && input[cur] == b'=' {
        cur += 1;
    } else {
        return Err(
            "Expected comparison operator (=, ~=, >=, <=) after attribute name".to_string(),
        );
    }

    // Parse value: everything until the matching ')'
    // Values can contain any character except unescaped ')' at our nesting level.
    // We just scan until we find ')'.
    while cur < input.len() && input[cur] != b')' {
        if input[cur] == b'\\' && cur + 1 < input.len() {
            // Skip escaped character
            cur += 2;
        } else {
            cur += 1;
        }
    }

    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_filter tests ----

    #[test]
    fn test_valid_filters() {
        assert!(validate_filter("(objectClass=*)").is_ok());
        assert!(validate_filter("(&(cn=admin)(objectClass=person))").is_ok());
        assert!(validate_filter("(|(cn=a)(cn=b))").is_ok());
    }

    #[test]
    fn test_invalid_filters() {
        assert!(validate_filter("").is_err());
        assert!(validate_filter("objectClass=*").is_err());
        assert!(validate_filter("((cn=admin)").is_err());
    }

    #[test]
    fn test_filter_whitespace() {
        // Trimmed — valid
        assert!(validate_filter("  (cn=test)  ").is_ok());
    }

    #[test]
    fn test_filter_nested() {
        assert!(validate_filter("(&(|(cn=a)(cn=b))(objectClass=person))").is_ok());
    }

    #[test]
    fn test_filter_negation() {
        assert!(validate_filter("(!(cn=admin))").is_ok());
    }

    #[test]
    fn test_filter_unbalanced_close() {
        assert!(validate_filter("(cn=admin))").is_err());
    }

    #[test]
    fn test_filter_presence() {
        assert!(validate_filter("(cn=*)").is_ok());
    }

    // ---- RFC 4515 specific error messages ----

    #[test]
    fn test_error_missing_open_paren() {
        let err = validate_filter("cn=admin").unwrap_err();
        assert!(err.contains("Expected '('"), "got: {}", err);
    }

    #[test]
    fn test_error_missing_close_paren() {
        let err = validate_filter("(cn=admin").unwrap_err();
        assert!(err.contains("Expected ')'"), "got: {}", err);
    }

    #[test]
    fn test_error_missing_operator() {
        let err = validate_filter("(cn)").unwrap_err();
        assert!(err.contains("comparison operator"), "got: {}", err);
    }

    #[test]
    fn test_error_empty_attr_name() {
        let err = validate_filter("(=value)").unwrap_err();
        assert!(err.contains("Expected attribute name"), "got: {}", err);
    }

    #[test]
    fn test_error_empty_and_list() {
        let err = validate_filter("(&)").unwrap_err();
        assert!(err.contains("Empty filter list"), "got: {}", err);
    }

    #[test]
    fn test_error_empty_or_list() {
        let err = validate_filter("(|)").unwrap_err();
        assert!(err.contains("Empty filter list"), "got: {}", err);
    }

    #[test]
    fn test_valid_comparison_operators() {
        assert!(validate_filter("(cn=value)").is_ok());
        assert!(validate_filter("(cn~=value)").is_ok());
        assert!(validate_filter("(cn>=value)").is_ok());
        assert!(validate_filter("(cn<=value)").is_ok());
    }

    #[test]
    fn test_valid_complex_filter() {
        assert!(
            validate_filter("(&(objectClass=person)(|(cn=Alice)(cn=Bob))(!(sn=Smith)))").is_ok()
        );
    }

    #[test]
    fn test_valid_escaped_value() {
        assert!(validate_filter("(cn=test\\29value)").is_ok());
    }

    #[test]
    fn test_valid_attribute_with_options() {
        assert!(validate_filter("(cn;lang-en=test)").is_ok());
    }

    #[test]
    fn test_trailing_garbage() {
        let err = validate_filter("(cn=test)garbage").unwrap_err();
        assert!(err.contains("Unexpected characters"), "got: {}", err);
    }

    // ---- detect_attribute_context tests ----

    #[test]
    fn test_context_simple_attr() {
        assert_eq!(
            detect_attribute_context("(userPr"),
            Some("userPr".to_string())
        );
    }

    #[test]
    fn test_context_nested_attr() {
        assert_eq!(
            detect_attribute_context("(&(cn=admin)(obj"),
            Some("obj".to_string())
        );
    }

    #[test]
    fn test_context_value_position() {
        assert_eq!(detect_attribute_context("(cn=adm"), None);
    }

    #[test]
    fn test_context_empty_attr() {
        assert_eq!(detect_attribute_context("(&("), Some("".to_string()));
    }

    #[test]
    fn test_context_after_not() {
        assert_eq!(detect_attribute_context("(!(mem"), Some("mem".to_string()));
    }

    #[test]
    fn test_context_no_open_paren() {
        assert_eq!(detect_attribute_context("hello"), None);
    }

    #[test]
    fn test_context_all_matched() {
        // All parens are matched — no open context
        assert_eq!(detect_attribute_context("(cn=test)"), None);
    }

    #[test]
    fn test_context_boolean_operator_prefix() {
        assert_eq!(detect_attribute_context("(|obj"), Some("obj".to_string()));
    }

    // ---- detect_filter_context tests ----

    #[test]
    fn test_filter_context_empty_input() {
        assert_eq!(detect_filter_context(""), Some(FilterContext::Empty));
    }

    #[test]
    fn test_filter_context_just_open_paren() {
        assert_eq!(detect_filter_context("("), Some(FilterContext::Empty));
    }

    #[test]
    fn test_filter_context_attribute_name() {
        assert_eq!(
            detect_filter_context("(userPr"),
            Some(FilterContext::AttributeName {
                partial: "userPr".to_string()
            })
        );
    }

    #[test]
    fn test_filter_context_nested_attribute() {
        assert_eq!(
            detect_filter_context("(&(cn=admin)(obj"),
            Some(FilterContext::AttributeName {
                partial: "obj".to_string()
            })
        );
    }

    #[test]
    fn test_filter_context_value_equals() {
        assert_eq!(
            detect_filter_context("(cn=adm"),
            Some(FilterContext::Value {
                attr: "cn".to_string(),
                partial: "adm".to_string()
            })
        );
    }

    #[test]
    fn test_filter_context_value_objectclass() {
        assert_eq!(
            detect_filter_context("(objectClass=per"),
            Some(FilterContext::Value {
                attr: "objectClass".to_string(),
                partial: "per".to_string()
            })
        );
    }

    #[test]
    fn test_filter_context_value_approx() {
        assert_eq!(
            detect_filter_context("(cn~=val"),
            Some(FilterContext::Value {
                attr: "cn".to_string(),
                partial: "val".to_string()
            })
        );
    }

    #[test]
    fn test_filter_context_value_gte() {
        assert_eq!(
            detect_filter_context("(uidNumber>=100"),
            Some(FilterContext::Value {
                attr: "uidNumber".to_string(),
                partial: "100".to_string()
            })
        );
    }

    #[test]
    fn test_filter_context_value_empty() {
        assert_eq!(
            detect_filter_context("(cn="),
            Some(FilterContext::Value {
                attr: "cn".to_string(),
                partial: String::new()
            })
        );
    }

    #[test]
    fn test_filter_context_all_matched() {
        assert_eq!(detect_filter_context("(cn=test)"), None);
    }

    #[test]
    fn test_filter_context_empty_attr_after_bool() {
        assert_eq!(
            detect_filter_context("(&("),
            Some(FilterContext::AttributeName {
                partial: String::new()
            })
        );
    }

    #[test]
    fn test_filter_context_after_not() {
        assert_eq!(
            detect_filter_context("(!(mem"),
            Some(FilterContext::AttributeName {
                partial: "mem".to_string()
            })
        );
    }
}
