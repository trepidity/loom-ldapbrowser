/// Validate that a string looks like a valid LDAP filter.
pub fn validate_filter(filter: &str) -> Result<(), String> {
    let filter = filter.trim();
    if filter.is_empty() {
        return Err("Filter cannot be empty".to_string());
    }
    if !filter.starts_with('(') || !filter.ends_with(')') {
        return Err("Filter must be enclosed in parentheses".to_string());
    }

    let mut depth = 0i32;
    for ch in filter.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err("Unbalanced parentheses".to_string());
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("Unbalanced parentheses".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
