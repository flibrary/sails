use crate::{
    aead::AeadKey,
    guards::{AeadUserInfo, UserIdGuard, UserInfoGuard},
    recaptcha::ReCaptcha,
    smtp::SmtpCreds,
    DbConn, IntoFlash, Msg,
};
use askama::Template;
use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use rocket::{
    form::{Form, Strict},
    http::{Cookie, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    categories::{Categories, Category, CtgTrait},
    error::SailsDbError,
    products::*,
    users::*,
};

async fn send_verification_email(
    dst: &str,
    aead: &AeadKey,
    smtp: &SmtpCreds,
) -> anyhow::Result<()> {
    let uri = format!(
        "Your activation link: https://flibrary.info/user/activate?activation_key={}",
        base64::encode_config(
            aead.encrypt(dst.as_bytes())
                .map_err(|_| anyhow::anyhow!("mailaddress encryption failed"))?,
            base64::URL_SAFE
        )
    );

    let email = Message::builder()
        .from(smtp.smtp_username.parse()?)
        // We have already checked it once
        .to(dst.parse().unwrap())
        .subject("FLibrary registration verification link")
        .body(uri)?;

    let creds = Credentials::new(smtp.smtp_username.clone(), smtp.smtp_password.clone());

    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.smtp_server)?
            .credentials(creds)
            .build();

    mailer.send(email).await?;
    Ok(())
}

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
pub struct SignInPage {
    inner: Msg,
}

// This would be mounted under namespace `user` and eventually become `/user/signin`
#[get("/signin")]
pub async fn signin<'a>(flash: Option<FlashMessage<'_>>) -> SignInPage {
    SignInPage {
        inner: Msg::from_flash(flash),
    }
}

#[derive(Template)]
#[template(path = "user/signup.html")]
pub struct SignUpPage {
    inner: Msg,
    recaptcha_key: String,
}

#[get("/signup")]
pub async fn signup<'a>(
    flash: Option<FlashMessage<'_>>,
    recaptcha: &State<ReCaptcha>,
) -> SignUpPage {
    SignUpPage {
        inner: Msg::from_flash(flash),
        recaptcha_key: recaptcha.recaptcha_site_key().to_string(),
    }
}

#[post("/create_user", data = "<info>")]
pub async fn create_user(
    info: Form<Strict<SignUpForm>>,
    conn: DbConn,
    recaptcha: &State<ReCaptcha>,
    aead: &State<AeadKey>,
    smtp: &State<SmtpCreds>,
) -> Result<Redirect, Flash<Redirect>> {
    if !recaptcha
        .verify(&info.recaptcha_token)
        .await
        .map_err(|e| Flash::error(Redirect::to(uri!("/user", portal)), e.to_string()))?
        .success
    {
        return Err(Flash::error(
            Redirect::to(uri!("/user", portal)),
            "reCAPTCHA was unsuccessful".to_string(),
        ));
    };

    // We parse it to only allow outlook emails
    let email: lettre::Address = info
        .user_info
        .id
        .parse::<lettre::Address>()
        .into_flash(uri!("/user", signup))?;
    if email.domain() == "outlook.com" {
        send_verification_email(&info.user_info.id, &aead, &smtp)
            .await
            .into_flash(uri!("/user", signup))?;
        conn.run(move |c| info.user_info.to_ref()?.create(c))
            .await
            .into_flash(uri!("/user", signup))?;
        Ok(Redirect::to(uri!("/user", portal)))
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/user", portal)),
            "please use outlook email addresses",
        ))
    }
}

#[post("/update_user", data = "<info>")]
pub async fn update_user(
    user: UserIdGuard,
    info: Form<UserFormOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    if user.id.get_id() == info.id {
        conn.run(move |c| info.to_ref()?.update(c))
            .await
            .into_flash(uri!("/user", portal))?;

        Ok(Redirect::to(uri!("/user", portal)))
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/user", portal)),
            "not authorized to update",
        ))
    }
}

#[get("/activate")]
pub async fn activate_user(info: AeadUserInfo, conn: DbConn) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| info.info.set_validated(true).update(c))
        .await
        .into_flash(uri!("/user", portal))?;
    Ok(Redirect::to(uri!("/user", portal)))
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
        .into_flash(uri!("/user", portal))?;
    let mut cookie = Cookie::new("uid", user.get_id().to_string());
    cookie.set_secure(true);
    // Successfully validated, set private cookie.
    jar.add_private(cookie);
    Ok(Redirect::to(uri!("/user", portal)))
}

#[get("/logout")]
pub async fn logout(jar: &CookieJar<'_>) -> Redirect {
    if let Some(uid) = jar.get_private("uid") {
        jar.remove_private(uid);
    } else {
        // No UID specified, do nothing
    }
    // Redirect back to home
    Redirect::to("/")
}

#[derive(Template)]
#[template(path = "user/update_user_page.html")]
pub struct UpdateUserPage {
    user: UserInfo,
}

#[get("/update_user_page")]
pub async fn update_user_page(user: UserInfoGuard) -> UpdateUserPage {
    UpdateUserPage { user: user.info }
}

#[derive(Template)]
#[template(path = "user/portal.html")]
pub struct PortalPage {
    user: UserInfo,
    books: Vec<(ProductInfo, Option<Category>)>,
    inner: Msg,
}

// The flash message is required here because we may get error from update_user
#[get("/", rank = 1)]
pub async fn portal(
    flash: Option<FlashMessage<'_>>,
    user: UserInfoGuard,
    conn: DbConn,
) -> Result<PortalPage, Redirect> {
    let uid_cloned = user.info.get_id().to_string();
    let books = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, Option<Category>)>, SailsDbError> {
                ProductFinder::new(c, None)
                    .seller(&uid_cloned)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        let ctg = Categories::find_by_id(c, x.get_category_id()).ok();
                        Ok((x, ctg))
                    })
                    .collect()
            },
        )
        .await
        .unwrap(); // No error should be tolerated here (database error). 500 is expected
    Ok(PortalPage {
        user: user.info,
        books,
        inner: Msg::from_flash(flash),
    })
}

#[get("/", rank = 2)]
pub async fn portal_unsigned() -> Redirect {
    Redirect::to(uri!("/user", signin))
}
