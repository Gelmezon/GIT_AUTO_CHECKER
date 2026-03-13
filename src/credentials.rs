use aes_gcm::aead::{Aead, KeyInit, OsRng, rand_core::RngCore};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Clone)]
pub struct CredentialCipher {
    cipher: Aes256Gcm,
}

impl CredentialCipher {
    pub fn from_hex_key(hex_key: &str) -> Result<Self> {
        let key = hex::decode(hex_key.trim()).context("invalid credentials encryption key hex")?;
        if key.len() != 32 {
            bail!("credentials encryption key must be 32 bytes / 64 hex chars");
        }
        Ok(Self {
            cipher: Aes256Gcm::new_from_slice(&key)
                .map_err(|_| anyhow::anyhow!("failed to initialize credential cipher"))?,
        })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let mut nonce_bytes = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("failed to encrypt credential secret"))?;

        let mut output = nonce_bytes.to_vec();
        output.extend(ciphertext);
        Ok(STANDARD.encode(output))
    }

    pub fn decrypt(&self, ciphertext: &str) -> Result<String> {
        let decoded = STANDARD
            .decode(ciphertext.trim())
            .context("invalid encrypted credential payload")?;
        if decoded.len() < 13 {
            bail!("encrypted credential payload is too short");
        }

        let (nonce_bytes, encrypted) = decoded.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = self
            .cipher
            .decrypt(nonce, encrypted)
            .map_err(|_| anyhow::anyhow!("failed to decrypt credential secret"))?;
        String::from_utf8(plaintext).context("decrypted credential secret was not utf-8")
    }
}

pub fn generate_encryption_key_hex() -> String {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    hex::encode(key)
}
