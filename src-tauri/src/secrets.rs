//! The Anthropic API key lives in the OS keychain (macOS Keychain, Windows Credential
//! Manager), never in a file.

const SERVICE: &str = "Säljtipset";
const ACCOUNT: &str = "anthropic-api-key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("Nyckelringen är inte tillgänglig: {e}"))
}

pub fn get_api_key() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Kunde inte läsa API-nyckeln: {e}")),
    }
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    entry()?.set_password(key).map_err(|e| format!("Kunde inte spara API-nyckeln: {e}"))
}

pub fn delete_api_key() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Kunde inte ta bort API-nyckeln: {e}")),
    }
}
