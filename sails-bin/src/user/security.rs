use crate::{aead::AeadKey, guards::UserGuard};
use chacha20poly1305::Nonce;
use chrono::{offset::Utc, Duration};
use rand::{prelude::StdRng, RngCore, SeedableRng};

// 1. Should expire
// 2. Should not be guessable
// 3. Should invalidate once used
// challenge = chacha20poly1305(plain = hashed_passwd, nonce = exipration datetime, key = app secret)
// The basic idea is that the challenge cannot be guessed/coined (even with user himself)
// And the challenge plaintext is kept safe throughout the process. Altering the exipration date fails the challenge as nonce is incorrect.
// NOTE: Expiration time should be set in correct timezone, otherwise time-based attack could be possible.
pub(super) fn generate_passwd_reset_link(
    dst: &str,
    hashed_passwd: &str,
    aead: &AeadKey,
) -> anyhow::Result<String> {
    let mut nonce = [0u8; 12];
    StdRng::from_entropy().fill_bytes(&mut nonce);
    let nonce = Nonce::clone_from_slice(&nonce);

    // Concat expiration date with hashed_password (first eight bytes are exp date)
    let mut exp = (Utc::now() + Duration::minutes(30))
        .timestamp()
        .to_be_bytes()
        .to_vec();
    exp.extend_from_slice(hashed_passwd.as_bytes());

    let challenge = base64::encode_config(
        aead.encrypt(&exp, &nonce)
            .map_err(|_| anyhow::anyhow!("password reset link encryption failed"))?,
        base64::URL_SAFE,
    );
    Ok(uri!(
        "https://flibrary.info/user",
        super::reset_passwd_now(
            dst,
            base64::encode_config(&nonce, base64::URL_SAFE),
            challenge
        )
    )
    .to_string())
}

pub(super) fn generate_verification_link(dst: &str, aead: &AeadKey) -> anyhow::Result<String> {
    let mut nonce = [0u8; 12];
    StdRng::from_entropy().fill_bytes(&mut nonce);
    let nonce = Nonce::clone_from_slice(&nonce);

    Ok(uri!(
        "https://flibrary.info/user",
        super::activate_user(
            base64::encode_config(
                aead.encrypt(dst.as_bytes(), &nonce,)
                    .map_err(|_| anyhow::anyhow!("mailaddress encryption failed"))?,
                base64::URL_SAFE
            ),
            base64::encode_config(&nonce, base64::URL_SAFE)
        )
    )
    .to_string())
}
