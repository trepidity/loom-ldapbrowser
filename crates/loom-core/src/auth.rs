use crate::connection::LdapConnection;
use crate::error::CoreError;
use tracing::{error, info};

impl LdapConnection {
    /// Perform a simple bind with the given DN and password.
    pub async fn simple_bind(&mut self, bind_dn: &str, password: &str) -> Result<(), CoreError> {
        let result = self
            .ldap
            .simple_bind(bind_dn, password)
            .await
            .map_err(CoreError::Ldap)?;

        if result.rc != 0 {
            error!(
                "Bind failed for DN '{}': rc={}, {}",
                bind_dn, result.rc, result.text
            );
            return Err(CoreError::BindFailed(format!(
                "LDAP bind returned rc={}: {}",
                result.rc, result.text
            )));
        }

        info!("Bound as {}", bind_dn);
        self.store_credentials(bind_dn.to_string(), password.to_string());
        Ok(())
    }

    /// Perform an anonymous bind.
    pub async fn anonymous_bind(&mut self) -> Result<(), CoreError> {
        let result = self
            .ldap
            .simple_bind("", "")
            .await
            .map_err(CoreError::Ldap)?;

        if result.rc != 0 {
            error!("Anonymous bind failed: rc={}, {}", result.rc, result.text);
            return Err(CoreError::BindFailed(format!(
                "Anonymous bind returned rc={}: {}",
                result.rc, result.text
            )));
        }

        info!("Bound anonymously");
        Ok(())
    }
}
