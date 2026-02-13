use std::collections::HashSet;

use ldap3::Mod;
use tracing::info;

use crate::connection::LdapConnection;
use crate::error::CoreError;

impl LdapConnection {
    /// Modify an entry's attributes.
    pub async fn modify_entry(
        &mut self,
        dn: &str,
        mods: Vec<Mod<String>>,
    ) -> Result<(), CoreError> {
        let result = self.ldap.modify(dn, mods).await.map_err(CoreError::Ldap)?;

        if result.rc != 0 {
            return Err(CoreError::ModifyFailed(format!(
                "Modify {} failed rc={}: {}",
                dn, result.rc, result.text
            )));
        }

        info!("Modified entry: {}", dn);
        Ok(())
    }

    /// Replace a single attribute value.
    pub async fn replace_attribute_value(
        &mut self,
        dn: &str,
        attr: &str,
        old_value: &str,
        new_value: &str,
    ) -> Result<(), CoreError> {
        let mods = vec![
            Mod::Delete(attr.to_string(), HashSet::from([old_value.to_string()])),
            Mod::Add(attr.to_string(), HashSet::from([new_value.to_string()])),
        ];
        self.modify_entry(dn, mods).await
    }

    /// Add a value to an attribute.
    pub async fn add_attribute_value(
        &mut self,
        dn: &str,
        attr: &str,
        value: &str,
    ) -> Result<(), CoreError> {
        let mods = vec![Mod::Add(
            attr.to_string(),
            HashSet::from([value.to_string()]),
        )];
        self.modify_entry(dn, mods).await
    }

    /// Delete a specific value from an attribute.
    pub async fn delete_attribute_value(
        &mut self,
        dn: &str,
        attr: &str,
        value: &str,
    ) -> Result<(), CoreError> {
        let mods = vec![Mod::Delete(
            attr.to_string(),
            HashSet::from([value.to_string()]),
        )];
        self.modify_entry(dn, mods).await
    }

    /// Delete an entry by DN.
    pub async fn delete_entry(&mut self, dn: &str) -> Result<(), CoreError> {
        let result = self.ldap.delete(dn).await.map_err(CoreError::Ldap)?;

        if result.rc != 0 {
            return Err(CoreError::ModifyFailed(format!(
                "Delete {} failed rc={}: {}",
                dn, result.rc, result.text
            )));
        }

        info!("Deleted entry: {}", dn);
        Ok(())
    }
}
