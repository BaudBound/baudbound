use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use baudbound_storage::SecretCipher;
use keyring::{Entry, Error};

const SECRET_KEY_ENVIRONMENT_VARIABLE: &str = "BAUDBOUND_SECRET_KEY";
const KEYRING_SERVICE: &str = "app.baudbound.runner";
const KEYRING_USERNAME: &str = "database-key-v1";
const KEY_LENGTH: usize = 32;

pub(crate) fn headless_secret_cipher_from_environment() -> Result<Option<SecretCipher>> {
    let Some(encoded) = std::env::var_os(SECRET_KEY_ENVIRONMENT_VARIABLE) else {
        return Ok(None);
    };
    let encoded = encoded.into_string().map_err(|_| {
        anyhow::anyhow!("{SECRET_KEY_ENVIRONMENT_VARIABLE} must contain UTF-8 base64")
    })?;
    let bytes = STANDARD
        .decode(encoded.trim())
        .with_context(|| format!("{SECRET_KEY_ENVIRONMENT_VARIABLE} must be valid base64"))?;
    let key: [u8; KEY_LENGTH] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        anyhow::anyhow!(
            "{SECRET_KEY_ENVIRONMENT_VARIABLE} must decode to {KEY_LENGTH} bytes, found {}",
            bytes.len()
        )
    })?;
    if key.iter().all(|byte| *byte == 0) {
        bail!("{SECRET_KEY_ENVIRONMENT_VARIABLE} must not contain an all-zero key");
    }
    Ok(Some(SecretCipher::from_key(key)))
}

pub(crate) fn desktop_secret_cipher() -> Result<SecretCipher> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .context("failed to open the operating-system credential vault")?;
    let key = match entry.get_secret() {
        Ok(bytes) => bytes,
        Err(Error::NoEntry) => {
            let key = SecretCipher::generate_key()?;
            entry.set_secret(&key).context(
                "failed to store the runner secret key in the operating-system credential vault",
            )?;
            key.to_vec()
        }
        Err(error) => {
            return Err(error).context(
                "failed to read the runner secret key from the operating-system credential vault",
            );
        }
    };
    let key: [u8; KEY_LENGTH] = key.try_into().map_err(|key: Vec<u8>| {
        anyhow::anyhow!(
            "the operating-system credential vault contains an invalid BaudBound key length: expected {KEY_LENGTH}, found {}",
            key.len()
        )
    })?;
    if key.iter().all(|byte| *byte == 0) {
        bail!("the operating-system credential vault contains an invalid all-zero BaudBound key");
    }
    Ok(SecretCipher::from_key(key))
}

pub(crate) fn generate_environment_secret_key() -> Result<String> {
    SecretCipher::generate_key()
        .map(|key| STANDARD.encode(key))
        .map_err(Into::into)
}
