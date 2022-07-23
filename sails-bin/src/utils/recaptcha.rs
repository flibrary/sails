use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReCaptcha {
    site_key: String,
    secret_key: String,
}

impl ReCaptcha {
    /// Get a reference to the re captcha's recaptcha site key.
    pub fn site_key(&self) -> &str {
        &self.site_key
    }

    /// Get a reference to the re captcha's recaptcha secret key.
    pub fn secret_key(&self) -> &str {
        &self.secret_key
    }

    pub async fn verify(&self, token: &str) -> Result<ReCaptchaResponse, reqwest::Error> {
        let params = [("secret", self.secret_key()), ("response", token)];
        let client = reqwest::Client::new();
        client
            .post("https://www.recaptcha.net/recaptcha/api/siteverify")
            .form(&params)
            .send()
            .await?
            .json::<ReCaptchaResponse>()
            .await
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReCaptchaResponse {
    pub success: bool,
}
