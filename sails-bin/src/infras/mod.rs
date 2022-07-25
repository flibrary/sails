// Rocket basics, i.e. struct fairing and flash msgs.
pub mod basics;
// Rocket-based database infra
pub mod database;
// Rocket-based i18n infra
pub mod i18n;
// Alipay API interop
pub mod alipay;
// Rocket-based encryption-related infra
pub mod aead;
// Rocket-based image hosting infra
pub mod images;
// Rocket-based OpenID Connect infra
pub mod oidc;
// Rocket-based Google ReCaptcha infra
pub mod recaptcha;
// Rocket-based Google mailbox infra
pub mod smtp;
// Rocket-based Telegram bot infra
pub mod tg_bot;
// Digital content hosting
pub mod digicons;

// Permission-related request guards
pub mod guards;
