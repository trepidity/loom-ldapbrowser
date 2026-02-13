use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A single LDAP entry with its DN and attributes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapEntry {
    pub dn: String,
    pub attributes: BTreeMap<String, Vec<String>>,
}

impl LdapEntry {
    pub fn new(dn: String, attributes: BTreeMap<String, Vec<String>>) -> Self {
        Self { dn, attributes }
    }

    pub fn from_search_entry(entry: ldap3::SearchEntry) -> Self {
        Self {
            dn: entry.dn,
            attributes: entry.attrs.into_iter().collect(),
        }
    }

    /// Get the first value of an attribute, if present.
    pub fn first_value(&self, attr: &str) -> Option<&str> {
        self.attributes
            .get(attr)
            .and_then(|vals| vals.first())
            .map(|s| s.as_str())
    }

    /// Get the RDN (first component of the DN).
    pub fn rdn(&self) -> &str {
        self.dn.split(',').next().unwrap_or(&self.dn)
    }

    /// Get all object classes for this entry.
    pub fn object_classes(&self) -> Vec<&str> {
        self.attributes
            .get("objectClass")
            .map(|vals| vals.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_entry() {
        let entry = LdapEntry::new(
            "cn=Test,dc=example".to_string(),
            BTreeMap::from([("cn".to_string(), vec!["Test".to_string()])]),
        );
        assert_eq!(entry.dn, "cn=Test,dc=example");
        assert_eq!(entry.attributes.len(), 1);
    }

    #[test]
    fn test_first_value() {
        let entry = LdapEntry::new(
            "cn=Test,dc=example".to_string(),
            BTreeMap::from([
                ("cn".to_string(), vec!["Test".to_string()]),
                ("multi".to_string(), vec!["a".to_string(), "b".to_string()]),
            ]),
        );
        assert_eq!(entry.first_value("cn"), Some("Test"));
        assert_eq!(entry.first_value("multi"), Some("a"));
        assert_eq!(entry.first_value("missing"), None);
    }

    #[test]
    fn test_rdn() {
        let entry = LdapEntry::new(
            "cn=Admin,ou=Users,dc=example,dc=com".to_string(),
            BTreeMap::new(),
        );
        assert_eq!(entry.rdn(), "cn=Admin");
    }

    #[test]
    fn test_rdn_single_component() {
        let entry = LdapEntry::new("dc=com".to_string(), BTreeMap::new());
        assert_eq!(entry.rdn(), "dc=com");
    }

    #[test]
    fn test_object_classes() {
        let entry = LdapEntry::new(
            "cn=Test,dc=example".to_string(),
            BTreeMap::from([(
                "objectClass".to_string(),
                vec![
                    "top".to_string(),
                    "person".to_string(),
                    "inetOrgPerson".to_string(),
                ],
            )]),
        );
        assert_eq!(
            entry.object_classes(),
            vec!["top", "person", "inetOrgPerson"]
        );
    }

    #[test]
    fn test_object_classes_empty() {
        let entry = LdapEntry::new("cn=Test,dc=example".to_string(), BTreeMap::new());
        assert!(entry.object_classes().is_empty());
    }

    #[test]
    fn test_serialize_deserialize() {
        let entry = LdapEntry::new(
            "cn=Test,dc=example".to_string(),
            BTreeMap::from([("cn".to_string(), vec!["Test".to_string()])]),
        );
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LdapEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.dn, entry.dn);
        assert_eq!(deserialized.first_value("cn"), Some("Test"));
    }
}
