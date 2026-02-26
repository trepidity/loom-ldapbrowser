use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};

/// Information about a server certificate, extracted for display to the user.
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    pub host: String,
    pub port: u16,
    pub subject: String,
    pub issuer: String,
    pub not_before: String,
    pub not_after: String,
    pub fingerprint_sha256: String,
}

impl fmt::Display for CertificateInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{} subject={} issuer={} fingerprint={}",
            self.host, self.port, self.subject, self.issuer, self.fingerprint_sha256
        )
    }
}

/// A persistable entry for a trusted certificate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrustedCertEntry {
    pub host: String,
    pub port: u16,
    pub fingerprint_sha256: String,
    pub subject: String,
}

/// Store of trusted certificate fingerprints (session-only and permanent).
pub struct TrustStore {
    always_trusted: RwLock<HashMap<String, TrustedCertEntry>>,
    session_trusted: RwLock<HashSet<String>>,
}

impl fmt::Debug for TrustStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrustStore").finish_non_exhaustive()
    }
}

impl TrustStore {
    /// Create a trust store from previously saved config entries.
    pub fn from_config(entries: &[TrustedCertEntry]) -> Self {
        let mut map = HashMap::new();
        for entry in entries {
            map.insert(entry.fingerprint_sha256.clone(), entry.clone());
        }
        Self {
            always_trusted: RwLock::new(map),
            session_trusted: RwLock::new(HashSet::new()),
        }
    }

    /// Check if a fingerprint is trusted (either always or for this session).
    pub fn is_trusted(&self, fingerprint: &str) -> bool {
        let always = self.always_trusted.read().unwrap();
        if always.contains_key(fingerprint) {
            return true;
        }
        let session = self.session_trusted.read().unwrap();
        session.contains(fingerprint)
    }

    /// Add a certificate to the permanent trust store.
    pub fn trust_always(&self, entry: TrustedCertEntry) {
        let mut always = self.always_trusted.write().unwrap();
        always.insert(entry.fingerprint_sha256.clone(), entry);
    }

    /// Trust a certificate fingerprint for this session only.
    pub fn trust_session(&self, fingerprint: String) {
        let mut session = self.session_trusted.write().unwrap();
        session.insert(fingerprint);
    }

    /// Export the permanent trust entries for config serialization.
    pub fn to_config_entries(&self) -> Vec<TrustedCertEntry> {
        let always = self.always_trusted.read().unwrap();
        always.values().cloned().collect()
    }
}

/// Compute the SHA-256 fingerprint of DER-encoded certificate bytes.
/// Returns a colon-separated uppercase hex string (e.g. "AB:CD:EF:...").
pub fn sha256_fingerprint(der: &[u8]) -> String {
    let hash = Sha256::digest(der);
    hash.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(":")
}

/// Parse certificate info from DER bytes.
pub fn parse_cert_info(der: &[u8], host: &str, port: u16) -> CertificateInfo {
    let fingerprint = sha256_fingerprint(der);

    let (subject, issuer, not_before, not_after) =
        match x509_parser::parse_x509_certificate(der) {
            Ok((_, cert)) => {
                let subject = cert.subject().to_string();
                let issuer = cert.issuer().to_string();
                let not_before = cert
                    .validity()
                    .not_before
                    .to_rfc2822()
                    .unwrap_or_else(|_| cert.validity().not_before.to_string());
                let not_after = cert
                    .validity()
                    .not_after
                    .to_rfc2822()
                    .unwrap_or_else(|_| cert.validity().not_after.to_string());
                (subject, issuer, not_before, not_after)
            }
            Err(_) => (
                "Unknown".to_string(),
                "Unknown".to_string(),
                "Unknown".to_string(),
                "Unknown".to_string(),
            ),
        };

    CertificateInfo {
        host: host.to_string(),
        port,
        subject,
        issuer,
        not_before,
        not_after,
        fingerprint_sha256: fingerprint,
    }
}

/// Load the system's native root certificate store.
fn load_native_root_store() -> RootCertStore {
    let mut store = RootCertStore::empty();
    let certs_result = rustls_native_certs::load_native_certs();
    for cert in certs_result.certs {
        let _ = store.add(cert);
    }
    store
}

/// A rustls `ServerCertVerifier` that checks a trust store first,
/// then falls back to webpki verification. On failure, it captures the
/// certificate details into a shared slot for later inspection.
#[derive(Debug)]
pub struct CertCaptureVerifier {
    trust_store: Arc<TrustStore>,
    captured: Arc<Mutex<Option<CertificateInfo>>>,
    webpki_verifier: Arc<WebPkiServerVerifier>,
    host: String,
    port: u16,
}

impl CertCaptureVerifier {
    pub fn new(
        trust_store: Arc<TrustStore>,
        captured: Arc<Mutex<Option<CertificateInfo>>>,
        host: &str,
        port: u16,
    ) -> Self {
        let root_store = load_native_root_store();
        let webpki_verifier = WebPkiServerVerifier::builder(Arc::new(root_store))
            .build()
            .expect("failed to build webpki verifier");

        Self {
            trust_store,
            captured,
            webpki_verifier,
            host: host.to_string(),
            port,
        }
    }
}

impl ServerCertVerifier for CertCaptureVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let fingerprint = sha256_fingerprint(end_entity.as_ref());

        // Check trust store first
        if self.trust_store.is_trusted(&fingerprint) {
            return Ok(ServerCertVerified::assertion());
        }

        // Try standard webpki verification
        match self.webpki_verifier.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        ) {
            Ok(verified) => Ok(verified),
            Err(err) => {
                // Capture the certificate info for the UI
                let cert_info = parse_cert_info(end_entity.as_ref(), &self.host, self.port);
                if let Ok(mut slot) = self.captured.lock() {
                    *slot = Some(cert_info);
                }
                Err(err)
            }
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.webpki_verifier
            .verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.webpki_verifier
            .verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.webpki_verifier.supported_verify_schemes()
    }
}

/// Build a rustls `ClientConfig` that uses our `CertCaptureVerifier`.
pub fn build_client_config(
    trust_store: Arc<TrustStore>,
    captured: Arc<Mutex<Option<CertificateInfo>>>,
    host: &str,
    port: u16,
) -> Arc<ClientConfig> {
    let verifier = CertCaptureVerifier::new(trust_store, captured, host, port);
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Arc::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_fingerprint() {
        let data = b"hello world";
        let fp = sha256_fingerprint(data);
        // SHA-256 of "hello world" is well-known
        assert!(fp.contains(":"));
        assert_eq!(fp.len(), 32 * 3 - 1); // XX:XX:... format
        assert_eq!(
            fp,
            "B9:4D:27:B9:93:4D:3E:08:A5:2E:52:D7:DA:7D:AB:FA:C4:84:EF:E3:7A:53:80:EE:90:88:F7:AC:E2:EF:CD:E9"
        );
    }

    #[test]
    fn test_trust_store_session() {
        let store = TrustStore::from_config(&[]);
        assert!(!store.is_trusted("AA:BB"));
        store.trust_session("AA:BB".to_string());
        assert!(store.is_trusted("AA:BB"));
    }

    #[test]
    fn test_trust_store_always() {
        let entry = TrustedCertEntry {
            host: "ldap.example.com".to_string(),
            port: 636,
            fingerprint_sha256: "CC:DD".to_string(),
            subject: "CN=ldap.example.com".to_string(),
        };
        let store = TrustStore::from_config(&[]);
        store.trust_always(entry);
        assert!(store.is_trusted("CC:DD"));
        let entries = store.to_config_entries();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_trust_store_from_config() {
        let entries = vec![TrustedCertEntry {
            host: "ldap.example.com".to_string(),
            port: 636,
            fingerprint_sha256: "EE:FF".to_string(),
            subject: "CN=ldap.example.com".to_string(),
        }];
        let store = TrustStore::from_config(&entries);
        assert!(store.is_trusted("EE:FF"));
        assert!(!store.is_trusted("00:11"));
    }
}
