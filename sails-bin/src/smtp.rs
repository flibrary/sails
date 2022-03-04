use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmtpCreds {
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_server: String,
}

impl SmtpCreds {
    pub async fn send(&self, dst: &str, subject: &str, body: String) -> anyhow::Result<()> {
        let email = Message::builder()
            .from(Mailbox::new(
                Some("FLibrary Sails".to_string()),
                self.smtp_username.parse()?,
            ))
            // We have already checked it once
            .to(dst.parse()?)
            .subject(subject)
            .body(body)?;

        let creds = Credentials::new(self.smtp_username.clone(), self.smtp_password.clone());

        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.smtp_server)?
                .credentials(creds)
                .build();

        mailer.send(email).await?;
        Ok(())
    }
}
