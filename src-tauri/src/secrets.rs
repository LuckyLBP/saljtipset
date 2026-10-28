//! The Anthropic API key lives in the OS keychain (macOS Keychain, Windows Credential
//! Manager), never in a file.

const SERVICE: &str = "Bubbe";
/// Before 0.2.0 the app was called Säljtipset; a key saved then is moved over on first read.
const OLD_SERVICE: &str = "Säljtipset";
const ACCOUNT: &str = "anthropic-api-key";

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("Nyckelringen är inte tillgänglig: {e}"))
}

pub fn get_api_key() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(migrate_old_key()),
        Err(e) => Err(format!("Kunde inte läsa API-nyckeln: {e}")),
    }
}

fn migrate_old_key() -> Option<String> {
    let old = keyring::Entry::new(OLD_SERVICE, ACCOUNT).ok()?;
    let key = old.get_password().ok()?;
    if entry().ok()?.set_password(&key).is_ok() {
        let _ = old.delete_credential();
    }
    Some(key)
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
