use std::path::Path;

use crate::entry::LdapEntry;
use crate::error::CoreError;

/// Export entries to JSON format (array of entry objects).
pub fn export(entries: &[LdapEntry], path: &Path) -> Result<usize, CoreError> {
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| CoreError::ExportError(format!("JSON serialization failed: {}", e)))?;

    std::fs::write(path, json)
        .map_err(|e| CoreError::ExportError(format!("Failed to write file: {}", e)))?;

    Ok(entries.len())
}

/// Serialize entries to a JSON string.
pub fn to_string(entries: &[LdapEntry]) -> Result<String, CoreError> {
    serde_json::to_string_pretty(entries)
        .map_err(|e| CoreError::ExportError(format!("JSON serialization failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_export_json_roundtrip() {
        let entries = vec![LdapEntry::new(
            "cn=Test,dc=example,dc=com".to_string(),
            BTreeMap::from([
                ("cn".to_string(), vec!["Test".to_string()]),
                ("sn".to_string(), vec!["User".to_string()]),
            ]),
        )];

        let json = to_string(&entries).unwrap();
        assert!(json.contains("cn=Test,dc=example,dc=com"));

        let parsed: Vec<LdapEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].dn, "cn=Test,dc=example,dc=com");
    }
}
