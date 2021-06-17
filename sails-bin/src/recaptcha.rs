use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReCaptcha {
    recaptcha_site_key: String,
    recaptcha_secret_key: String,
}

impl ReCaptcha {
    /// Get a reference to the re captcha's recaptcha site key.
    pub fn recaptcha_site_key(&self) -> &str {
        &self.recaptcha_site_key
    }

    /// Get a reference to the re captcha's recaptcha secret key.
    pub fn recaptcha_secret_key(&self) -> &str {
        &self.recaptcha_secret_key
    }

    pub async fn verify(&self, token: &str) -> Result<ReCaptchaResponse, reqwest::Error> {
        let params = [("secret", self.recaptcha_secret_key()), ("response", token)];
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
