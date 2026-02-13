/// Get the parent DN (everything after the first comma).
pub fn parent_dn(dn: &str) -> Option<&str> {
    dn.find(',').map(|i| &dn[i + 1..])
}

/// Get the RDN (first component before the first comma).
pub fn rdn(dn: &str) -> &str {
    dn.split(',').next().unwrap_or(dn)
}

/// Get the depth of a DN (number of components).
pub fn depth(dn: &str) -> usize {
    if dn.is_empty() {
        0
    } else {
        dn.split(',').count()
    }
}

/// Check if `ancestor` is an ancestor of `dn`.
pub fn is_ancestor(dn: &str, ancestor: &str) -> bool {
    if ancestor.is_empty() {
        return true;
    }
    let dn_lower = dn.to_lowercase();
    let ancestor_lower = ancestor.to_lowercase();
    dn_lower.ends_with(&ancestor_lower) && dn_lower.len() > ancestor_lower.len()
}

/// Get the display name from an RDN (the value part after '=').
pub fn rdn_display_name(dn: &str) -> &str {
    let r = rdn(dn);
    r.find('=').map(|i| &r[i + 1..]).unwrap_or(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_dn() {
        assert_eq!(
            parent_dn("cn=admin,dc=example,dc=com"),
            Some("dc=example,dc=com")
        );
        assert_eq!(parent_dn("dc=com"), None);
    }

    #[test]
    fn test_rdn() {
        assert_eq!(rdn("cn=admin,dc=example,dc=com"), "cn=admin");
        assert_eq!(rdn("dc=com"), "dc=com");
    }

    #[test]
    fn test_depth() {
        assert_eq!(depth("cn=admin,dc=example,dc=com"), 3);
        assert_eq!(depth("dc=com"), 1);
        assert_eq!(depth(""), 0);
    }

    #[test]
    fn test_is_ancestor() {
        assert!(is_ancestor(
            "cn=admin,dc=example,dc=com",
            "dc=example,dc=com"
        ));
        assert!(!is_ancestor("dc=example,dc=com", "dc=example,dc=com"));
    }

    #[test]
    fn test_rdn_display_name() {
        assert_eq!(rdn_display_name("cn=admin,dc=example,dc=com"), "admin");
        assert_eq!(rdn_display_name("dc=com"), "com");
    }

    #[test]
    fn test_parent_dn_empty() {
        assert_eq!(parent_dn(""), None);
    }

    #[test]
    fn test_rdn_empty() {
        assert_eq!(rdn(""), "");
    }

    #[test]
    fn test_is_ancestor_empty_ancestor() {
        assert!(is_ancestor("dc=example,dc=com", ""));
    }

    #[test]
    fn test_is_ancestor_case_insensitive() {
        assert!(is_ancestor(
            "cn=Admin,DC=EXAMPLE,DC=COM",
            "dc=example,dc=com"
        ));
    }

    #[test]
    fn test_depth_deeply_nested() {
        assert_eq!(depth("cn=user,ou=people,ou=dept,dc=example,dc=com"), 5);
    }

    #[test]
    fn test_rdn_display_name_no_equals() {
        assert_eq!(rdn_display_name("nodots"), "nodots");
    }
}
