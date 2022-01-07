use chacha20poly1305::{
    aead,
    aead::{Aead, NewAead},
    ChaCha20Poly1305, Key, Nonce,
};
use serde::{Deserialize, Deserializer};

fn deserialize_aead_key<'de, D>(deserializer: D) -> Result<ChaCha20Poly1305, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;

    Ok(ChaCha20Poly1305::new(&Key::clone_from_slice(
        buf.as_bytes(),
    )))
}

#[derive(Clone, Deserialize)]
pub struct AeadKey {
    #[serde(deserialize_with = "deserialize_aead_key")]
    aead_key: ChaCha20Poly1305,
}

impl AeadKey {
    pub fn encrypt(&self, plain: &[u8], nonce: &Nonce) -> Result<Vec<u8>, aead::Error> {
        self.aead_key.encrypt(nonce, plain)
    }

    pub fn decrypt(&self, cipher: &[u8], nonce: &Nonce) -> Result<Vec<u8>, aead::Error> {
        self.aead_key.decrypt(nonce, cipher)
    }
}
