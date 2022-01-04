use core::fmt;
use rocket::serde::DeserializeOwned;
use rsa::{
    pkcs1::FromRsaPrivateKey, pkcs8::FromPublicKey, Hash, PaddingScheme::PKCS1v15Sign, PublicKey,
    RsaPrivateKey, RsaPublicKey,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::{borrow::Borrow, fmt::Display};

pub trait BizContent: Serialize {
    fn method(&self) -> &'static str;
}

fn deserialize_rsa_pubkey<'de, D>(deserializer: D) -> Result<RsaPublicKey, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;

    // Base64 decode
    let decoded = base64::decode(&buf).expect("failed to decode base64 content");
    Ok(RsaPublicKey::from_public_key_der(&decoded).unwrap())
}

fn deserialize_rsa_privkey<'de, D>(deserializer: D) -> Result<RsaPrivateKey, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;

    // Base64 decode
    let decoded = base64::decode(&buf).expect("failed to decode base64 content");
    Ok(RsaPrivateKey::from_pkcs1_der(&decoded).unwrap())
}

// The public key of alipay used for response validation
// Not used in our usecases because we do syncronized HTTPS requests.
#[derive(Clone, Debug, Deserialize)]
pub struct AlipayPubKey {
    #[serde(deserialize_with = "deserialize_rsa_pubkey")]
    alipay_pubkey: RsaPublicKey,
}

impl AlipayPubKey {
    #[allow(unused)]
    fn verify(&self, content: &str, sig: &str) -> Result<(), anyhow::Error> {
        let mut sh = Sha256::new();
        sh.update(content);
        let hashed: &[u8] = &sh.finalize();
        let sig = base64::decode(sig)?;
        Ok(self.alipay_pubkey.verify(
            PKCS1v15Sign {
                hash: Some(Hash::SHA2_256),
            },
            hashed,
            &sig,
        )?)
    }
}

// The private key of app used for request used to sign request
#[derive(Clone, Debug, Deserialize)]
pub struct AlipayAppPrivKey {
    #[serde(deserialize_with = "deserialize_rsa_privkey")]
    alipay_app_privkey: RsaPrivateKey,
}

impl AlipayAppPrivKey {
    fn sign<T: Display>(&self, content: &T) -> Result<String, anyhow::Error> {
        let mut sh = Sha256::new();
        sh.update(content.to_string());
        let hashed: &[u8] = &sh.finalize();
        let res = self.alipay_app_privkey.sign(
            PKCS1v15Sign {
                hash: Some(Hash::SHA2_256),
            },
            hashed,
        )?;
        Ok(base64::encode(res))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct AlipayClient {
    alipay_app_id: String,
}

impl AlipayClient {
    pub fn request<'a, B: BizContent>(
        &'a self,
        private_key: &'a AlipayAppPrivKey,
        biz_content: B,
    ) -> anyhow::Result<Request<'a, B>> {
        use chrono::{FixedOffset, Utc};

        let mut req = Request {
            app_id: &self.alipay_app_id,
            charset: "utf-8",
            method: biz_content.method(),
            sign_type: "RSA2",
            sign: None,
            timestamp: Utc::now()
                .with_timezone(&FixedOffset::east(8 * 3600))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
            version: "1.0",
            biz_content,
        };

        req.sign = Some(private_key.borrow().sign(&req)?);
        Ok(req)
    }
}

fn serialize_biz_content<S>(x: impl Serialize, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&serde_json::to_string(&x).unwrap())
}

// Fields are ordered alphabetically to sign correctly
#[derive(Clone, Debug, Serialize)]
pub struct Request<'a, B: BizContent> {
    app_id: &'a str,
    #[serde(serialize_with = "serialize_biz_content")]
    biz_content: B,
    charset: &'a str,
    method: &'a str,
    sign_type: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    sign: Option<String>,
    // timestamp is not stored anywhere
    timestamp: String,
    version: &'a str,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response<T> {
    code: String,
    msg: String,
    sub_msg: Option<String>,
    // If the request errored, there could be no content. In order to preserve the errored response, let's use option
    #[serde(flatten)]
    content: Option<T>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SignedResponse<T> {
    #[serde(
        alias = "alipay_trade_query_response",
        alias = "alipay_trade_precreate_response"
    )]
    response: Response<T>,
    sign: String,
}

impl<T> Display for SignedResponse<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}, {}, {:?}",
            self.response.code, self.response.msg, self.response.sub_msg
        )
    }
}

impl<'a, B: BizContent> Request<'a, B> {
    // On success, it returns the URL to the QR code. On failure, it returns the full response for debug
    pub async fn send<T: DeserializeOwned>(&self) -> anyhow::Result<Result<T, SignedResponse<T>>> {
        let client = reqwest::Client::new();
        let res = client
            // The official alipay SDK concats charset after gateway as well. Blame their documentations.
            // For reference: https://github.com/yansongda/pay/issues/14
            .post("https://openapi.alipay.com/gateway.do?charset=utf-8")
            .form(&self)
            .send()
            .await?
            .text()
            .await?;

        let res = serde_json::from_str::<SignedResponse<T>>(&res)?;
        // There is no need to verify the response as it is secured by HTTPS
        if (res.response.code == "10000") || (res.response.msg == "Success") {
            Ok(Ok(res.response.content.unwrap()))
        } else {
            Ok(Err(res))
        }
    }
}

impl<'a, B: BizContent> Display for Request<'a, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(sig) = &self.sign {
            write!(
                f,
                r#"app_id={}&biz_content={}&charset={}&method={}&sign={}&sign_type={}&timestamp={}&version={}"#,
                self.app_id,
                serde_json::to_string(&self.biz_content).map_err(|_| fmt::Error)?,
                self.charset,
                self.method,
                sig,
                self.sign_type,
                self.timestamp,
                self.version
            )
        } else {
            write!(
                f,
                r#"app_id={}&biz_content={}&charset={}&method={}&sign_type={}&timestamp={}&version={}"#,
                self.app_id,
                serde_json::to_string(&self.biz_content).map_err(|_| fmt::Error)?,
                self.charset,
                self.method,
                self.sign_type,
                self.timestamp,
                self.version
            )
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Precreate<'a> {
    out_trade_no: &'a str,
    subject: &'a str,
    total_amount: i64,
}

impl<'a> Precreate<'a> {
    pub fn new(out_trade_no: &'a str, subject: &'a str, total_amount: i64) -> Self {
        Self {
            out_trade_no,
            subject,
            total_amount,
        }
    }
}

impl<'a> BizContent for Precreate<'a> {
    fn method(&self) -> &'static str {
        "alipay.trade.precreate"
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct TradeQuery<'a> {
    out_trade_no: &'a str,
}

impl<'a> TradeQuery<'a> {
    pub fn new(out_trade_no: &'a str) -> Self {
        Self { out_trade_no }
    }
}

impl<'a> BizContent for TradeQuery<'a> {
    fn method(&self) -> &'static str {
        "alipay.trade.query"
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct PrecreateResp {
    pub qr_code: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TradeQueryResp {
    pub trade_status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Generate test private key
    fn priv_key() -> AlipayAppPrivKey {
        let decoded = base64::decode(
            "MIIEpgIBAAKCAQEAryYXsAzhfM+s8JKvauMBO34WSppYTnlI8VY5cSeJ4zPjgtJcZzq2GLJy/BztQj30+QzDOLvH4a7cPylF35EAyrgvyNDU5nVVVEZFUMldScjltZZiAEt5wmrNpB2ruAuPas9pfPzC8g1Wz5c/JlLryzXuMnQ65YplJ+knvujU5xsrHeNJN5PMqUVpvzEv9FXDRlVYBf3tiJlEwL19xdy5vowdQOkU9YoAEUwHdQWd9t2uOmp/Rz3jOMUnbpW9X/hDjsUfweJQskp5Gmg9g+Dz9b6be8Uejr2V3L8GGSZux3C+457RbCz1kHJzub8KsRqqTXRIt5QDLAMpLMnI4uW3CQIDAQABAoIBAQCs2mUqQ6wNZ09/pOQmEp5Wvlr1iVl5KM1KEBRkjebRKZZwt3amEhVATmyYT1v/sfGgEG5iAUCEg/OtCeiBeTNU3W2bPC3Auy9ZGnix/+hnNzMsgJt7OwGRU6JbQ0UDP7VsbgHnbfReubdg7B1QjylRxYmslXTCsFCgkMO+7z2eTVZjmTrAT/0XPddwptM0S5QLf6P3Kl3zdgOHrLNV0B/BTzZC55SeJEEEdlv+WEqy/1oAPW6WoyeynZoU4VsDk/mYtf3cBsOUKD6+jmz1XPSYl42s/rL8RKqbzn1uXy7snFu0yDB5Cf3YtF9BGrqpJnWrj+kBL4wmPT2z8Yz7P+P5AoGBAPCYrwCC8wfE+ybjvc27RPdlLk0IH4scPMFPdh+sij3fv55v9k/nyhUDbIKNozzF4kImHemTJdlNvueoxfBmkimeNUkWI4l1xtvEGdB80n8Qwps9yPykBfAoMarH4eKEUb4fLuogEBYg9ovxtVttiJiJK6akC2kXmNilrDcm0nWHAoGBALpcvQq1TEZMx4gGyFkhePCVFmvRlaF31P4/LTdxP7dm46TUO/DEiawWHcUVyZ4oBVZPwGkScIARPQap7OacY2GmNxKYkeMtPJZiMx38WqxhZtgQyEITiMvNxMXPUmlul2oECzB+NEsLcmf8Pq0acyFAKDjh35xcRcIQdJ6QRZLvAoGBAMaO225qo7M3x6XyscPF10bsw+di2tVtel52+59sP7KMo5FYCUksm8P3zWd5CMyw6ud4mZsYi1XpKbH5wVGC9QFPxd4JXU6mWnUoQ72iJf4jkNeZh/OoUhY+ta6hwzOzy9pB1e/2ghAhKBeaZPeNT/vCyLmADMKwbL3vDE9/xJSlAoGBAIqL+3aEhioVVpmIAVZSDik9jSem7ojWH6DMsv7u0KG2ejLGHbHHS1qGLqegpP1RX3ZlX/Q9Yymypx3XImnnhfLIsVS/3GV58fsTElGOlJJm7yBeiaKmByMM3Ob6VJhRQXvteawZhyLrZahs3OOwMDteCQkQ0z7ZUnsN5MUlGLQ9AoGBAMrZQpZ2+EZ42vWgtrwLTJhLdHdYux3d+gJggFqw+XS8+H0c2bd0ssYsfPrZFfsy4F7tdpgcWD1kS4XpYevycVCQtugYV5eDYASi2+rtsFmoVO6jeuFW/E7ryOTT2MGt4MQ7+P7rB3aRUuf4T1xr+8iJdClHwEw49rnVLuQhNvk3",
        )
        .unwrap();
        AlipayAppPrivKey {
            alipay_app_privkey: RsaPrivateKey::from_pkcs1_der(&decoded).unwrap(),
        }
    }

    // Generate test public key
    // Note: even though it returns `AlipayPubKey`, it's actually app pubkey.
    fn app_pub_key() -> AlipayPubKey {
        let decoded = base64::decode(
            "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAryYXsAzhfM+s8JKvauMBO34WSppYTnlI8VY5cSeJ4zPjgtJcZzq2GLJy/BztQj30+QzDOLvH4a7cPylF35EAyrgvyNDU5nVVVEZFUMldScjltZZiAEt5wmrNpB2ruAuPas9pfPzC8g1Wz5c/JlLryzXuMnQ65YplJ+knvujU5xsrHeNJN5PMqUVpvzEv9FXDRlVYBf3tiJlEwL19xdy5vowdQOkU9YoAEUwHdQWd9t2uOmp/Rz3jOMUnbpW9X/hDjsUfweJQskp5Gmg9g+Dz9b6be8Uejr2V3L8GGSZux3C+457RbCz1kHJzub8KsRqqTXRIt5QDLAMpLMnI4uW3CQIDAQAB",
        ).unwrap();
        AlipayPubKey {
            alipay_pubkey: RsaPublicKey::from_public_key_der(&decoded).unwrap(),
        }
    }

    #[test]
    fn precreate_request() {
        let client = AlipayClient {
            alipay_app_id: "2021003109657615".to_string(),
        };

        let priv_key = priv_key();
        let mut req = client
            .request(&priv_key, Precreate::new("12345", "AP PreCalculus", 100))
            .unwrap();

        // println!("{}", req);

        // Seperate the signature from text
        let sig = req.sign.unwrap();
        req.sign = None;

        // Verify signature
        assert!(app_pub_key().verify(&req.to_string(), &sig).is_ok());
    }

    #[test]
    fn trade_query() {
        let client = AlipayClient {
            alipay_app_id: "2021003109657615".to_string(),
        };

        let priv_key = priv_key();
        let mut req = client.request(&priv_key, TradeQuery::new("12345")).unwrap();

        // let client = reqwest::blocking::Client::new();
        // dbg!(client
        //     .post("https://openapi.alipay.com/gateway.do?charset=utf-8")
        //     .form(&req)
        //     .send()
        //     .unwrap()
        //     .text());

        // Seperate the signature from text
        let sig = req.sign.unwrap();
        req.sign = None;

        // Verify signature
        assert!(app_pub_key().verify(&req.to_string(), &sig).is_ok());
    }
}
