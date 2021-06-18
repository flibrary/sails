use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmtpCreds {
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_server: String,
}
