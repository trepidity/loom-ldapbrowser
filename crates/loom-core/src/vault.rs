use std::collections::HashMap;
use std::path::{Path, PathBuf};

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use rand::RngCore;
use zeroize::Zeroize;

use crate::error::CoreError;

/// Magic bytes identifying a vault file.
const MAGIC: &[u8; 4] = b"LMVT";
/// Current vault file format version.
const VERSION: u8 = 0x01;
/// Salt length in bytes.
const SALT_LEN: usize = 32;
/// Nonce length in bytes.
const NONCE_LEN: usize = 12;
/// Derived key length in bytes (256-bit for ChaCha20).
const KEY_LEN: usize = 32;
/// Header size: magic(4) + version(1) + salt(32) + nonce(12) + ciphertext_len(4).
const HEADER_LEN: usize = 4 + 1 + SALT_LEN + NONCE_LEN + 4;

/// Argon2id parameters (OWASP minimums).
const ARGON2_M_COST: u32 = 65536; // 64 MB
const ARGON2_T_COST: u32 = 3;
const ARGON2_P_COST: u32 = 1;

/// An encrypted vault storing profile passwords.
///
/// The vault is a single encrypted file protected by a master password.
/// Plaintext is JSON: `{"ProfileName": "password", ...}`.
pub struct Vault {
    path: PathBuf,
    salt: [u8; SALT_LEN],
    master_key: Vec<u8>,
    entries: HashMap<String, String>,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("path", &self.path)
            .field("entries", &format!("{} entries", self.entries.len()))
            .finish()
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        self.master_key.zeroize();
        for v in self.entries.values_mut() {
            v.zeroize();
        }
    }
}

impl Vault {
    /// Create a new empty vault at `path`, encrypted with `password`.
    pub fn create(path: &Path, password: &str) -> Result<Self, CoreError> {
        let mut salt = [0u8; SALT_LEN];
        rand::thread_rng().fill_bytes(&mut salt);

        let master_key = derive_key(password, &salt)?;
        let entries = HashMap::new();

        let vault = Vault {
            path: path.to_path_buf(),
            salt,
            master_key,
            entries,
        };
        vault.save()?;
        Ok(vault)
    }

    /// Open an existing vault at `path` with the given `password`.
    pub fn open(path: &Path, password: &str) -> Result<Self, CoreError> {
        let data = std::fs::read(path)
            .map_err(|e| CoreError::VaultError(format!("Failed to read vault file: {}", e)))?;

        if data.len() < HEADER_LEN {
            return Err(CoreError::VaultError("Vault file is too small".to_string()));
        }

        // Validate magic
        if &data[0..4] != MAGIC {
            return Err(CoreError::VaultError(
                "Not a valid vault file (bad magic)".to_string(),
            ));
        }

        // Validate version
        if data[4] != VERSION {
            return Err(CoreError::VaultError(format!(
                "Unsupported vault version: {}",
                data[4]
            )));
        }

        // Read salt
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&data[5..5 + SALT_LEN]);

        // Read nonce
        let mut nonce_bytes = [0u8; NONCE_LEN];
        nonce_bytes.copy_from_slice(&data[5 + SALT_LEN..5 + SALT_LEN + NONCE_LEN]);

        // Read ciphertext length
        let ct_len_offset = 5 + SALT_LEN + NONCE_LEN;
        let ct_len = u32::from_be_bytes([
            data[ct_len_offset],
            data[ct_len_offset + 1],
            data[ct_len_offset + 2],
            data[ct_len_offset + 3],
        ]) as usize;

        let ct_start = HEADER_LEN;
        if data.len() < ct_start + ct_len {
            return Err(CoreError::VaultError("Vault file is truncated".to_string()));
        }

        let ciphertext = &data[ct_start..ct_start + ct_len];

        // Derive key and decrypt
        let master_key = derive_key(password, &salt)?;

        let cipher = ChaCha20Poly1305::new_from_slice(&master_key)
            .map_err(|e| CoreError::VaultError(format!("Cipher init failed: {}", e)))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CoreError::VaultError("Wrong password or corrupted vault".to_string()))?;

        let entries: HashMap<String, String> = serde_json::from_slice(&plaintext)
            .map_err(|e| CoreError::VaultError(format!("Failed to parse vault data: {}", e)))?;

        Ok(Vault {
            path: path.to_path_buf(),
            salt,
            master_key,
            entries,
        })
    }

    /// Check if a vault file exists at `path`.
    pub fn exists(path: &Path) -> bool {
        path.is_file()
    }

    /// Default vault file path: `<config_dir>/loom-ldapbrowser/vault.dat`.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("loom-ldapbrowser")
            .join("vault.dat")
    }

    /// Look up the password for a profile.
    pub fn get_password(&self, profile_name: &str) -> Option<&str> {
        self.entries.get(profile_name).map(|s| s.as_str())
    }

    /// Store a password for a profile and persist to disk.
    pub fn set_password(&mut self, profile_name: &str, password: &str) -> Result<(), CoreError> {
        self.entries
            .insert(profile_name.to_string(), password.to_string());
        self.save()
    }

    /// Remove a profile's password and persist to disk.
    pub fn remove_password(&mut self, profile_name: &str) -> Result<(), CoreError> {
        self.entries.remove(profile_name);
        self.save()
    }

    /// Rename a profile key in the vault and persist to disk.
    pub fn rename_profile(&mut self, old_name: &str, new_name: &str) -> Result<(), CoreError> {
        if let Some(password) = self.entries.remove(old_name) {
            self.entries.insert(new_name.to_string(), password);
            self.save()?;
        }
        Ok(())
    }

    /// Encrypt and write the vault to disk.
    fn save(&self) -> Result<(), CoreError> {
        let plaintext = serde_json::to_vec(&self.entries)
            .map_err(|e| CoreError::VaultError(format!("Failed to serialize vault: {}", e)))?;

        let cipher = ChaCha20Poly1305::new_from_slice(&self.master_key)
            .map_err(|e| CoreError::VaultError(format!("Cipher init failed: {}", e)))?;

        // Fresh nonce every save
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_slice())
            .map_err(|e| CoreError::VaultError(format!("Encryption failed: {}", e)))?;

        let ct_len = ciphertext.len() as u32;

        // Build file
        let mut file_data = Vec::with_capacity(HEADER_LEN + ciphertext.len());
        file_data.extend_from_slice(MAGIC);
        file_data.push(VERSION);
        file_data.extend_from_slice(&self.salt);
        file_data.extend_from_slice(&nonce_bytes);
        file_data.extend_from_slice(&ct_len.to_be_bytes());
        file_data.extend_from_slice(&ciphertext);

        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::VaultError(format!("Failed to create vault directory: {}", e))
            })?;
        }

        std::fs::write(&self.path, &file_data)
            .map_err(|e| CoreError::VaultError(format!("Failed to write vault: {}", e)))?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(&self.path, perms);
        }

        Ok(())
    }
}

/// Derive a 256-bit key from a password and salt using Argon2id.
fn derive_key(password: &str, salt: &[u8]) -> Result<Vec<u8>, CoreError> {
    let params = argon2::Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_LEN))
        .map_err(|e| CoreError::VaultError(format!("Argon2 params error: {}", e)))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    let mut key = vec![0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| CoreError::VaultError(format!("Key derivation failed: {}", e)))?;

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        let vault = Vault::create(&path, "master123").unwrap();
        assert!(vault.entries.is_empty());
        assert!(path.exists());
        drop(vault);

        let vault = Vault::open(&path, "master123").unwrap();
        assert!(vault.entries.is_empty());
    }

    #[test]
    fn test_store_and_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        let mut vault = Vault::create(&path, "master123").unwrap();
        vault.set_password("MyServer", "s3cret").unwrap();
        drop(vault);

        let vault = Vault::open(&path, "master123").unwrap();
        assert_eq!(vault.get_password("MyServer"), Some("s3cret"));
        assert_eq!(vault.get_password("NonExistent"), None);
    }

    #[test]
    fn test_wrong_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        let mut vault = Vault::create(&path, "correct").unwrap();
        vault.set_password("Test", "pw").unwrap();
        drop(vault);

        let result = Vault::open(&path, "wrong");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Wrong password") || err.contains("corrupted"));
    }

    #[test]
    fn test_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        Vault::create(&path, "master123").unwrap();

        // Corrupt the file
        let mut data = std::fs::read(&path).unwrap();
        if data.len() > HEADER_LEN + 2 {
            data[HEADER_LEN + 1] ^= 0xFF;
        }
        std::fs::write(&path, &data).unwrap();

        let result = Vault::open(&path, "master123");
        assert!(result.is_err());
    }

    #[test]
    fn test_rename_profile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        let mut vault = Vault::create(&path, "master123").unwrap();
        vault.set_password("OldName", "pw123").unwrap();
        vault.rename_profile("OldName", "NewName").unwrap();

        assert_eq!(vault.get_password("OldName"), None);
        assert_eq!(vault.get_password("NewName"), Some("pw123"));
        drop(vault);

        // Verify persistence
        let vault = Vault::open(&path, "master123").unwrap();
        assert_eq!(vault.get_password("OldName"), None);
        assert_eq!(vault.get_password("NewName"), Some("pw123"));
    }

    #[test]
    fn test_remove_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        let mut vault = Vault::create(&path, "master123").unwrap();
        vault.set_password("Server1", "pw1").unwrap();
        vault.set_password("Server2", "pw2").unwrap();

        vault.remove_password("Server1").unwrap();
        assert_eq!(vault.get_password("Server1"), None);
        assert_eq!(vault.get_password("Server2"), Some("pw2"));
    }

    #[test]
    fn test_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        let mut vault = Vault::create(&path, "master123").unwrap();
        vault.set_password("Server1", "pw1").unwrap();
        vault.set_password("Server2", "pw2").unwrap();
        vault.set_password("Server3", "pw3").unwrap();
        drop(vault);

        let vault = Vault::open(&path, "master123").unwrap();
        assert_eq!(vault.get_password("Server1"), Some("pw1"));
        assert_eq!(vault.get_password("Server2"), Some("pw2"));
        assert_eq!(vault.get_password("Server3"), Some("pw3"));
    }

    #[test]
    fn test_overwrite_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        let mut vault = Vault::create(&path, "master123").unwrap();
        vault.set_password("Server", "old_pw").unwrap();
        vault.set_password("Server", "new_pw").unwrap();

        assert_eq!(vault.get_password("Server"), Some("new_pw"));
    }

    #[test]
    fn test_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        assert!(!Vault::exists(&path));
        Vault::create(&path, "master123").unwrap();
        assert!(Vault::exists(&path));
    }

    #[test]
    fn test_bad_magic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        Vault::create(&path, "master123").unwrap();

        let mut data = std::fs::read(&path).unwrap();
        data[0] = b'X';
        std::fs::write(&path, &data).unwrap();

        let result = Vault::open(&path, "master123");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bad magic"));
    }

    #[test]
    fn test_truncated_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vault.dat");

        std::fs::write(&path, b"LMV").unwrap(); // too short
        let result = Vault::open(&path, "master123");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too small"));
    }
}
