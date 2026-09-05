use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, Generate, KeyInit},
};
use anyhow::Context;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

pub struct CredentialEncryption {
    cipher: Aes256Gcm,
}

impl CredentialEncryption {
    pub fn from_hex_key(hex: &str) -> anyhow::Result<Self> {
        let key_bytes = hex::decode(hex.trim()).context("invalid hex master key")?;
        if key_bytes.len() != 32 {
            anyhow::bail!("master key must be 32 bytes (64 hex characters)");
        }
        let key = aes_gcm::Key::<Aes256Gcm>::try_from(&key_bytes[..])
            .map_err(|_| anyhow::anyhow!("invalid AES-256 key length"))?;
        Ok(Self {
            cipher: Aes256Gcm::new(&key),
        })
    }

    pub fn seal(&self, plaintext: &[u8]) -> anyhow::Result<String> {
        let nonce = Nonce::generate();
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("encrypt failed: {:?}", e))?;
        let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(&out))
    }

    pub fn unseal(&self, sealed: &str) -> anyhow::Result<Vec<u8>> {
        let bytes = BASE64
            .decode(sealed)
            .context("invalid base64 sealed payload")?;
        if bytes.len() < 12 {
            anyhow::bail!("sealed payload too short");
        }
        let (nonce, ciphertext) = bytes.split_at(12);
        let nonce = Nonce::try_from(nonce).map_err(|_| anyhow::anyhow!("invalid nonce length"))?;
        self.cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("decrypt failed: {:?}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let enc = CredentialEncryption::from_hex_key(key).unwrap();
        let plain = b"hf_very_secret_token";
        let sealed = enc.seal(plain).unwrap();
        assert_ne!(sealed, String::from_utf8_lossy(plain).to_string());
        let opened = enc.unseal(&sealed).unwrap();
        assert_eq!(opened, plain);
    }

    #[test]
    fn rejects_short_key() {
        let key = "0123456789abcdef";
        assert!(CredentialEncryption::from_hex_key(key).is_err());
    }
}
