use crate::aead::AeadKey;
use chacha20poly1305::Nonce;
use chrono::{offset::Utc, Duration};

// Convert i64 timestamp in secs to Nonce
// [u8; 8] + [u8; 4]
pub fn timestamp_to_nonce(exp: i64) -> Nonce {
    let mut exp_vec = exp.to_ne_bytes().to_vec();
    exp_vec.extend_from_slice(&[0u8, 0u8, 0u8, 0u8]);
    Nonce::clone_from_slice(&exp_vec)
}

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
    let exp = (Utc::now() + Duration::minutes(30)).timestamp();
    let challenge = base64::encode_config(
        aead.encrypt(hashed_passwd.as_bytes(), &timestamp_to_nonce(exp))
            .map_err(|_| anyhow::anyhow!("password reset link encryption failed"))?,
        base64::URL_SAFE,
    );
    Ok(format!(
        "https://flibrary.info/user/reset_passwd?user_id={}&exp={}&challenge={}",
        dst, exp, challenge
    ))
}

pub(super) fn generate_verification_link(dst: &str, aead: &AeadKey) -> anyhow::Result<String> {
    let exp = (Utc::now() + Duration::minutes(30)).timestamp();
    Ok(format!(
        "https://flibrary.info/user/activate?exp={}&enc_user_id={}",
        exp,
        base64::encode_config(
            aead.encrypt(
                dst.as_bytes(),
                &Nonce::clone_from_slice(&timestamp_to_nonce(exp)),
            )
            .map_err(|_| anyhow::anyhow!("mailaddress encryption failed"))?,
            base64::URL_SAFE
        )
    ))
}
