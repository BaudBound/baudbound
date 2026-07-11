use std::{fmt, sync::Arc};

use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use zeroize::Zeroizing;

use crate::StorageError;

const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;

#[derive(Clone)]
pub struct SecretCipher {
    key: Arc<Zeroizing<[u8; KEY_BYTES]>>,
}

impl SecretCipher {
    #[must_use]
    pub fn from_key(key: [u8; KEY_BYTES]) -> Self {
        Self {
            key: Arc::new(Zeroizing::new(key)),
        }
    }

    pub fn generate_key() -> Result<[u8; KEY_BYTES], StorageError> {
        let mut key = [0_u8; KEY_BYTES];
        getrandom::fill(&mut key).map_err(|source| {
            StorageError::SecretCrypto(format!("key generation failed: {source}"))
        })?;
        Ok(key)
    }

    pub(crate) fn encrypt(
        &self,
        plaintext: &[u8],
        associated_data: &[u8],
    ) -> Result<([u8; NONCE_BYTES], Vec<u8>), StorageError> {
        let mut nonce = [0_u8; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|source| {
            StorageError::SecretCrypto(format!("nonce generation failed: {source}"))
        })?;
        let key = Key::from(**self.key);
        let cipher = XChaCha20Poly1305::new(&key);
        let nonce_value = XNonce::from(nonce);
        let ciphertext = cipher
            .encrypt(
                &nonce_value,
                Payload {
                    msg: plaintext,
                    aad: associated_data,
                },
            )
            .map_err(|_| {
                StorageError::SecretCrypto("authenticated encryption failed".to_owned())
            })?;
        Ok((nonce, ciphertext))
    }

    pub(crate) fn decrypt(
        &self,
        nonce: &[u8],
        ciphertext: &[u8],
        associated_data: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, StorageError> {
        let nonce: &[u8; NONCE_BYTES] = nonce.try_into().map_err(|_| {
            StorageError::SecretCrypto("stored nonce has invalid length".to_owned())
        })?;
        let key = Key::from(**self.key);
        let cipher = XChaCha20Poly1305::new(&key);
        let nonce_value = XNonce::from(*nonce);
        cipher
            .decrypt(
                &nonce_value,
                Payload {
                    msg: ciphertext,
                    aad: associated_data,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| StorageError::SecretCrypto("secret authentication failed".to_owned()))
    }
}

impl fmt::Debug for SecretCipher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretCipher([redacted])")
    }
}
