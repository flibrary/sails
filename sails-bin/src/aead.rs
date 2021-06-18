use chacha20poly1305::{
    aead,
    aead::{Aead, NewAead},
    ChaCha20Poly1305, Key, Nonce,
};
use serde::{Deserialize, Deserializer};

fn deserialize_aead_key<'de, D>(deserializer: D) -> Result<(ChaCha20Poly1305, Nonce), D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;

    Ok((
        ChaCha20Poly1305::new(&Key::clone_from_slice(buf.as_bytes())),
        Nonce::clone_from_slice("unique nonce".as_ref()),
    ))
}

#[derive(Clone, Deserialize)]
pub struct AeadKey {
    #[serde(deserialize_with = "deserialize_aead_key")]
    aead_key: (ChaCha20Poly1305, Nonce),
}

impl AeadKey {
    pub fn encrypt(&self, plain: &[u8]) -> Result<Vec<u8>, aead::Error> {
        self.aead_key.0.encrypt(&self.aead_key.1, plain)
    }

    pub fn decrypt(&self, cipher: &[u8]) -> Result<Vec<u8>, aead::Error> {
        self.aead_key.0.decrypt(&self.aead_key.1, cipher)
    }
}
