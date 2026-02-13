use std::path::Path;

use crate::entry::LdapEntry;
use crate::error::CoreError;

/// Import entries from a JSON file (array of LdapEntry objects).
pub fn import(path: &Path) -> Result<Vec<LdapEntry>, CoreError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| CoreError::ImportError(format!("Failed to read file: {}", e)))?;
    parse_json(&content)
}

/// Parse a JSON string into entries.
pub fn parse_json(content: &str) -> Result<Vec<LdapEntry>, CoreError> {
    serde_json::from_str(content)
        .map_err(|e| CoreError::ImportError(format!("JSON parse failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json() {
        let json = r#"[
            {
                "dn": "cn=Test,dc=example,dc=com",
                "attributes": {
                    "cn": ["Test"],
                    "sn": ["User"]
                }
            }
        ]"#;

        let entries = parse_json(json).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].dn, "cn=Test,dc=example,dc=com");
        assert_eq!(entries[0].first_value("cn"), Some("Test"));
    }
}
