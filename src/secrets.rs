use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::STANDARD};
use baudbound_storage::SecretCipher;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use keyring::{Entry, Error};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

const SECRET_KEY_ENVIRONMENT_VARIABLE: &str = "BAUDBOUND_SECRET_KEY";
const KEYRING_SERVICE: &str = "app.baudbound.runner";
const KEYRING_USERNAME: &str = "database-key-v1";
const KEY_LENGTH: usize = 32;
const PASSWORD_KEY_FILE_NAME: &str = "password-secret-key.json";
const PASSWORD_KEY_PENDING_FILE_NAME: &str = "password-secret-key.pending.json";
const PASSWORD_KEY_FORMAT: &str = "baudbound.password-secret-key";
const PASSWORD_KEY_FORMAT_VERSION: u8 = 1;
const PASSWORD_KEY_AAD: &[u8] = b"baudbound-password-secret-key-v1";
const PASSWORD_MIN_BYTES: usize = 12;
const PASSWORD_MAX_BYTES: usize = 1024;
const PASSWORD_SALT_LENGTH: usize = 16;
const PASSWORD_NONCE_LENGTH: usize = 24;
const ARGON2_MEMORY_KIB: u32 = 65_536;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DesktopSecretStorageMode {
    OperatingSystem,
    Password,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PasswordKeyEnvelope {
    algorithm: String,
    ciphertext: String,
    format: String,
    format_version: u8,
    kdf: PasswordKeyKdf,
    nonce: String,
    salt: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PasswordKeyKdf {
    algorithm: String,
    iterations: u32,
    memory_kib: u32,
    parallelism: u32,
}

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

pub(crate) fn reset_desktop_secret_cipher() -> Result<SecretCipher> {
    let entry = Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .context("failed to open the operating-system credential vault")?;
    match entry.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => {}
        Err(error) => {
            return Err(error).context(
                "failed to reset the runner secret key in the operating-system credential vault",
            );
        }
    }
    let key = SecretCipher::generate_key()?;
    entry.set_secret(&key).context(
        "failed to store a new runner secret key in the operating-system credential vault",
    )?;
    Ok(SecretCipher::from_key(key))
}

pub(crate) fn password_secret_key_path(runner_home: &Path) -> PathBuf {
    runner_home.join(PASSWORD_KEY_FILE_NAME)
}

pub(crate) fn desktop_secret_storage_mode(runner_home: &Path) -> DesktopSecretStorageMode {
    if password_secret_key_path(runner_home).is_file() {
        DesktopSecretStorageMode::Password
    } else {
        DesktopSecretStorageMode::OperatingSystem
    }
}

pub(crate) fn unlock_password_secret_cipher(
    runner_home: &Path,
    password: &str,
) -> Result<SecretCipher> {
    validate_password(password)?;
    let path = password_secret_key_path(runner_home);
    let bytes = fs::read(&path).with_context(|| {
        format!(
            "failed to read password protected storage key {}",
            path.display()
        )
    })?;
    if bytes.len() > 16 * 1024 {
        bail!("password protected storage key is too large");
    }
    let envelope: PasswordKeyEnvelope = serde_json::from_slice(&bytes)
        .context("password protected storage key is not valid JSON")?;
    validate_envelope(&envelope)?;

    let salt = decode_exact::<PASSWORD_SALT_LENGTH>("salt", &envelope.salt)?;
    let nonce = decode_exact::<PASSWORD_NONCE_LENGTH>("nonce", &envelope.nonce)?;
    let ciphertext = STANDARD
        .decode(&envelope.ciphertext)
        .context("password protected storage key ciphertext is not valid base64")?;
    if ciphertext.len() != KEY_LENGTH + 16 {
        bail!("password protected storage key ciphertext has an invalid length");
    }

    let mut wrapping_key = Zeroizing::new([0_u8; KEY_LENGTH]);
    password_argon2()?
        .hash_password_into(password.as_bytes(), &salt, wrapping_key.as_mut())
        .map_err(|error| {
            anyhow::anyhow!("failed to derive the password protected storage key: {error}")
        })?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to initialize protected storage encryption"))?;
    let nonce = XNonce::from(nonce);
    let mut plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                msg: &ciphertext,
                aad: PASSWORD_KEY_AAD,
            },
        )
        .map_err(|_| {
            anyhow::anyhow!("the password is incorrect or the protected storage key is damaged")
        })?;
    let key: [u8; KEY_LENGTH] = plaintext.as_slice().try_into().map_err(|_| {
        anyhow::anyhow!("password protected storage key contains an invalid key length")
    })?;
    plaintext.zeroize();
    Ok(SecretCipher::from_key(key))
}

pub(crate) fn prepare_password_secret_cipher(
    runner_home: &Path,
    password: &str,
) -> Result<SecretCipher> {
    validate_password(password)?;
    fs::create_dir_all(runner_home).with_context(|| {
        format!(
            "failed to create the runner storage directory {}",
            runner_home.display()
        )
    })?;
    discard_pending_password_secret_storage(runner_home);

    let key = SecretCipher::generate_key()?;
    let salt = random_bytes::<PASSWORD_SALT_LENGTH>()?;
    let nonce = random_bytes::<PASSWORD_NONCE_LENGTH>()?;
    let mut wrapping_key = Zeroizing::new([0_u8; KEY_LENGTH]);
    password_argon2()?
        .hash_password_into(password.as_bytes(), &salt, wrapping_key.as_mut())
        .map_err(|error| {
            anyhow::anyhow!("failed to derive the password protected storage key: {error}")
        })?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| anyhow::anyhow!("failed to initialize protected storage encryption"))?;
    let nonce_value = XNonce::from(nonce);
    let ciphertext = cipher
        .encrypt(
            &nonce_value,
            Payload {
                msg: &key,
                aad: PASSWORD_KEY_AAD,
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to protect the runner secret key"))?;
    let envelope = PasswordKeyEnvelope {
        algorithm: "xchacha20poly1305".to_owned(),
        ciphertext: STANDARD.encode(ciphertext),
        format: PASSWORD_KEY_FORMAT.to_owned(),
        format_version: PASSWORD_KEY_FORMAT_VERSION,
        kdf: PasswordKeyKdf {
            algorithm: "argon2id-v1.3".to_owned(),
            iterations: ARGON2_ITERATIONS,
            memory_kib: ARGON2_MEMORY_KIB,
            parallelism: ARGON2_PARALLELISM,
        },
        nonce: STANDARD.encode(nonce),
        salt: STANDARD.encode(salt),
    };
    write_private_file(
        &runner_home.join(PASSWORD_KEY_PENDING_FILE_NAME),
        &serde_json::to_vec_pretty(&envelope)
            .context("failed to encode the password protected storage key")?,
    )?;
    Ok(SecretCipher::from_key(key))
}

pub(crate) fn commit_password_secret_storage(runner_home: &Path) -> Result<()> {
    let pending = runner_home.join(PASSWORD_KEY_PENDING_FILE_NAME);
    let final_path = password_secret_key_path(runner_home);
    fs::rename(&pending, &final_path).with_context(|| {
        format!(
            "failed to activate password protected storage key {}",
            final_path.display()
        )
    })
}

pub(crate) fn discard_pending_password_secret_storage(runner_home: &Path) {
    let path = runner_home.join(PASSWORD_KEY_PENDING_FILE_NAME);
    if let Err(error) = fs::remove_file(&path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, path = %path.display(), "failed to remove pending secret key");
    }
}

pub(crate) fn remove_password_secret_storage(runner_home: &Path) -> Result<()> {
    let path = password_secret_key_path(runner_home);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to remove password protected storage key {}",
                path.display()
            )
        }),
    }
}

pub(crate) fn generate_environment_secret_key() -> Result<String> {
    SecretCipher::generate_key()
        .map(|key| STANDARD.encode(key))
        .map_err(Into::into)
}

fn validate_password(password: &str) -> Result<()> {
    if password.len() < PASSWORD_MIN_BYTES {
        bail!("storage password must contain at least {PASSWORD_MIN_BYTES} bytes");
    }
    if password.len() > PASSWORD_MAX_BYTES {
        bail!("storage password must contain at most {PASSWORD_MAX_BYTES} bytes");
    }
    if password.contains('\0') {
        bail!("storage password must not contain null characters");
    }
    Ok(())
}

fn validate_envelope(envelope: &PasswordKeyEnvelope) -> Result<()> {
    if envelope.format != PASSWORD_KEY_FORMAT
        || envelope.format_version != PASSWORD_KEY_FORMAT_VERSION
        || envelope.algorithm != "xchacha20poly1305"
        || envelope.kdf.algorithm != "argon2id-v1.3"
        || envelope.kdf.memory_kib != ARGON2_MEMORY_KIB
        || envelope.kdf.iterations != ARGON2_ITERATIONS
        || envelope.kdf.parallelism != ARGON2_PARALLELISM
    {
        bail!("password protected storage key uses an unsupported format");
    }
    Ok(())
}

fn password_argon2() -> Result<Argon2<'static>> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        Some(KEY_LENGTH),
    )
    .map_err(|error| anyhow::anyhow!("invalid password key derivation parameters: {error}"))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

fn decode_exact<const LENGTH: usize>(label: &str, value: &str) -> Result<[u8; LENGTH]> {
    let bytes = STANDARD
        .decode(value)
        .with_context(|| format!("password protected storage key {label} is not valid base64"))?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        anyhow::anyhow!(
            "password protected storage key {label} has an invalid length: expected {LENGTH}, found {}",
            bytes.len()
        )
    })
}

fn random_bytes<const LENGTH: usize>() -> Result<[u8; LENGTH]> {
    let mut bytes = [0_u8; LENGTH];
    getrandom::fill(&mut bytes).context("failed to generate protected storage randomness")?;
    Ok(bytes)
}

fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(contents)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to persist {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_storage_round_trip_and_wrong_password_rejection() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        prepare_password_secret_cipher(directory.path(), "a sufficiently long password")
            .expect("protected key should be prepared");
        commit_password_secret_storage(directory.path())
            .expect("protected key should be committed");

        assert_eq!(
            desktop_secret_storage_mode(directory.path()),
            DesktopSecretStorageMode::Password
        );
        unlock_password_secret_cipher(directory.path(), "a sufficiently long password")
            .expect("correct password should unlock the key");
        let error = unlock_password_secret_cipher(directory.path(), "a different long password")
            .expect_err("wrong password should be rejected");
        assert!(
            error
                .to_string()
                .contains("password is incorrect or the protected storage key is damaged")
        );
    }

    #[test]
    fn password_storage_rejects_short_passwords() {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        let error = prepare_password_secret_cipher(directory.path(), "too short")
            .expect_err("short password should be rejected");
        assert!(error.to_string().contains("at least 12 bytes"));
        assert!(
            !directory
                .path()
                .join(PASSWORD_KEY_PENDING_FILE_NAME)
                .exists()
        );
    }
}
