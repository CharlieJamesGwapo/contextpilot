//! OS-backed credential storage. Discovery never calls these functions.
//! Values and underlying OS error messages must never be logged.
use crate::Provider;
use std::fmt;
const SERVICE: &str = "ContextPilot";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    InvalidKey,
    VaultUnavailable,
}
impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self { Self::InvalidKey=>"API key must be nonempty printable ASCII without whitespace and at most 8192 bytes", Self::VaultUnavailable=>"Secure OS credential storage is unavailable or access was denied; use the provider environment variable instead" })
    }
}
impl std::error::Error for CredentialError {}
fn validate(key: &str) -> Result<(), CredentialError> {
    if key.is_empty() || key.len() > 8192 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        Err(CredentialError::InvalidKey)
    } else {
        Ok(())
    }
}
fn entry(provider: Provider) -> Result<keyring::Entry, CredentialError> {
    keyring::Entry::new(SERVICE, provider.credential_env())
        .map_err(|_| CredentialError::VaultUnavailable)
}
pub fn set(provider: Provider, key: &str) -> Result<(), CredentialError> {
    validate(key)?;
    entry(provider)?
        .set_password(key)
        .map_err(|_| CredentialError::VaultUnavailable)
}
pub fn read(provider: Provider) -> Result<Option<String>, CredentialError> {
    match entry(provider)?.get_password() {
        Ok(key) => {
            validate(&key)?;
            Ok(Some(key))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(CredentialError::VaultUnavailable),
    }
}
pub fn remove(provider: Provider) -> Result<(), CredentialError> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(CredentialError::VaultUnavailable),
    }
}
/// Resolve environment first; only an explicitly requested provider operation may reach the vault.
pub(crate) fn load(provider: Provider) -> Result<String, crate::ProviderError> {
    if let Some(key) = std::env::var(provider.credential_env())
        .ok()
        .filter(|k| !k.trim().is_empty())
    {
        validate(&key).map_err(|_| crate::ProviderError::InvalidCredential)?;
        return Ok(key);
    }
    read(provider)
        .map_err(|_| crate::ProviderError::CredentialStorage)?
        .ok_or(crate::ProviderError::MissingCredential)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_keys_rejected_before_any_vault_access() {
        for key in ["", " ", "a\nb", "a b", "a\0b", "é"] {
            assert_eq!(set(Provider::OpenAi, key), Err(CredentialError::InvalidKey));
        }
        assert_eq!(
            validate(&"a".repeat(8193)),
            Err(CredentialError::InvalidKey)
        );
        assert!(validate("fake-test-key_123").is_ok());
    }
    #[test]
    fn safe_error_messages_contain_no_values() {
        assert!(!CredentialError::VaultUnavailable
            .to_string()
            .contains("fake-test-key"));
    }
}
