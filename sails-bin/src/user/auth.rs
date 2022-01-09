use crate::{
    aead::AeadKey, guards::*, recaptcha::ReCaptcha, sanitize_html, smtp::SmtpCreds, DbConn,
    IntoFlash,
};
use askama::Template;
use chrono::{offset::Utc, DateTime, NaiveDateTime};
use rand::{prelude::StdRng, RngCore, SeedableRng};
use rocket::{
    form::{Form, Strict},
    http::{Cookie as HttpCookie, CookieJar},
    response::{Flash, Redirect},
    State,
};
use sails_db::users::*;

use super::generate_verification_link;

#[derive(FromForm)]
pub struct SignUpForm {
    user_info: UserFormOwned,
    #[field(name = "g-recaptcha-response")]
    recaptcha_token: String,
}

// Form used for validating an user
#[derive(FromForm)]
pub struct Validation {
    email: String,
    password: String,
}

#[derive(Template)]
#[template(path = "user/signin.html")]
pub struct SignInPage;

// This would be mounted under namespace `user` and eventually become `/user/signin`
#[get("/signin")]
pub async fn signin<'a>() -> SignInPage {
    SignInPage
}

#[derive(Template)]
#[template(path = "user/signup.html")]
pub struct SignUpPage {
    recaptcha_key: String,
}

#[get("/signup")]
pub async fn signup<'a>(recaptcha: &State<ReCaptcha>) -> SignUpPage {
    SignUpPage {
        recaptcha_key: recaptcha.recaptcha_site_key().to_string(),
    }
}

#[post("/create_user", data = "<info>")]
pub async fn create_user(
    mut info: Form<Strict<SignUpForm>>,
    conn: DbConn,
    recaptcha: &State<ReCaptcha>,
    aead: &State<AeadKey>,
    smtp: &State<SmtpCreds>,
) -> Result<Redirect, Flash<Redirect>> {
    if !recaptcha
        .verify(&info.recaptcha_token)
        .await
        .into_flash(uri!("/"))?
        .success
    {
        return Err(Flash::error(
            Redirect::to(uri!("/")),
            "reCAPTCHA was unsuccessful".to_string(),
        ));
    };

    // We parse it to only allow outlook emails
    let email: lettre::Address = info
        .user_info
        .id
        .parse::<lettre::Address>()
        .into_flash(uri!("/"))?;
    if email.domain() == "outlook.com" {
        smtp.send(
            &info.user_info.id,
            "Your FLibrary Verification Email",
            generate_verification_link(&info.user_info.id, aead).into_flash(uri!("/"))?,
        )
        .await
        .into_flash(uri!("/"))?;
        // Sanitize the html
        // Even though we didn't give user a chance to type in description,
        // malicious users can still manually post the form to us.
        info.user_info.description = info
            .user_info
            .description
            .as_ref()
            .map(|d| sanitize_html(d));
        conn.run(move |c| info.user_info.to_ref()?.create(c))
            .await
            .into_flash(uri!("/"))?;
        Ok(Redirect::to(uri!("/user", signup_instruction)))
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/")),
            "please use outlook email addresses",
        ))
    }
}

#[derive(Template)]
#[template(path = "user/signup_instruction.html")]
pub struct SignUpInstruction;

#[get("/signup_instruction")]
pub async fn signup_instruction() -> SignUpInstruction {
    SignUpInstruction
}

#[derive(Template)]
#[template(path = "user/reset_passwd_instruction.html")]
pub struct ResetPasswdInstruction;

#[get("/reset_passwd_instruction")]
pub async fn reset_passwd_instruction() -> ResetPasswdInstruction {
    ResetPasswdInstruction
}

#[derive(Template)]
#[template(path = "user/reset_passwd_confirmation.html")]
pub struct ResetPasswdConfirmation {
    reset_passwd: String,
}

#[derive(Template)]
#[template(path = "user/reset_passwd.html")]
pub struct ResetPasswd {
    recaptcha_key: String,
}

// We validate the challenge based on expiration time and the AEAD encrypted hashed password.
// Then we reset the user password to a CSPRNG-generated u32 number.
#[get("/reset_passwd?<exp>&<challenge>", rank = 1)]
pub async fn reset_passwd_now(
    conn: DbConn,
    user_info: UserInfoGuard<Param>,
    aead: &State<AeadKey>,
    exp: i64,
    challenge: String,
) -> Result<ResetPasswdConfirmation, Flash<Redirect>> {
    if Utc::now() <= DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(exp, 0), Utc) {
        // Within expiration time
        let decoded = base64::decode_config(&challenge, base64::URL_SAFE).into_flash(uri!("/"))?;

        let extracted_hashed_passwd = String::from_utf8(
            aead.decrypt(&decoded, &super::timestamp_to_nonce(exp))
                .map_err(|_| anyhow::anyhow!("reset password link decryption failed"))
                .into_flash(uri!("/"))?,
        )
        .into_flash(uri!("/"))?;

        if user_info.info.get_hashed_passwd() == extracted_hashed_passwd {
            // We can finally reset it.
            let rand_passwd = StdRng::from_entropy().next_u32().to_string();
            let rand_passwd_clone = rand_passwd.clone();
            conn.run(move |c| user_info.info.set_password(&rand_passwd_clone)?.update(c))
                .await
                .into_flash(uri!("/"))?;
            Ok(ResetPasswdConfirmation {
                reset_passwd: rand_passwd,
            })
        } else {
            Err(Flash::error(
                Redirect::to(uri!("/")),
                "reset password link challenge failed",
            ))
        }
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/")),
            "reset password link expired",
        ))
    }
}

#[get("/reset_passwd", rank = 2)]
pub async fn reset_passwd_page(recaptcha: &State<ReCaptcha>) -> ResetPasswd {
    ResetPasswd {
        recaptcha_key: recaptcha.recaptcha_site_key().to_string(),
    }
}

#[derive(FromForm)]
pub struct ResetPasswdForm {
    user_id: String,
    #[field(name = "g-recaptcha-response")]
    recaptcha_token: String,
}

#[post("/reset_passwd", data = "<form>")]
pub async fn reset_passwd_post(
    form: Form<ResetPasswdForm>,
    conn: DbConn,
    recaptcha: &State<ReCaptcha>,
    aead: &State<AeadKey>,
    smtp: &State<SmtpCreds>,
) -> Result<Redirect, Flash<Redirect>> {
    if !recaptcha
        .verify(&form.recaptcha_token)
        .await
        .into_flash(uri!("/"))?
        .success
    {
        return Err(Flash::error(
            Redirect::to(uri!("/")),
            "reCAPTCHA was unsuccessful".to_string(),
        ));
    };

    let user = conn
        .run(move |c| UserFinder::new(c, None).id(&form.user_id).first_info())
        .await
        .into_flash(uri!("/"))?;

    smtp.send(
        user.get_id(),
        "Your FLibrary Password Reset Link",
        super::generate_passwd_reset_link(user.get_id(), user.get_hashed_passwd(), aead)
            .into_flash(uri!("/"))?,
    )
    .await
    .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/user", reset_passwd_instruction)))
}

#[derive(Template)]
#[template(path = "user/email_verified.html")]
pub struct EmailVerified;

#[get("/email_verified")]
pub async fn email_verified() -> EmailVerified {
    EmailVerified
}

#[get("/activate")]
pub async fn activate_user(
    info: UserInfoGuard<Aead>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| info.info.set_validated(true).update(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/user", email_verified)))
}

#[post("/validate", data = "<info>")]
pub async fn validate(
    jar: &CookieJar<'_>,
    info: Form<Validation>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let user = conn
        .run(move |c| UserId::login(c, &info.email, &info.password))
        .await
        .into_flash(uri!("/"))?;
    let mut cookie = HttpCookie::new("uid", user.get_id().to_string());
    cookie.set_secure(true);
    // Successfully validated, set private cookie.
    jar.add_private(cookie);
    Ok(Redirect::to(uri!("/user", super::portal)))
}

#[get("/logout")]
pub async fn logout(jar: &CookieJar<'_>) -> Redirect {
    if let Some(uid) = jar.get_private("uid") {
        jar.remove_private(uid);
    } else {
        // No UID specified, do nothing
    }
    // Redirect back to home
    Redirect::to(uri!("/"))
}

#[derive(Template)]
#[template(path = "user/change_passwd.html")]
pub struct ChangePassword;

#[get("/change_passwd")]
pub async fn change_passwd_page() -> ChangePassword {
    ChangePassword
}

#[post("/change_passwd", data = "<password>")]
pub async fn change_passwd_post(
    user: UserInfoGuard<Cookie>,
    password: Form<String>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| user.info.set_password(password.into_inner())?.update(c))
        .await
        .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/user", super::portal)))
}
