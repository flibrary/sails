use crate::{
    aead::AeadKey, guards::*, recaptcha::ReCaptcha, sanitize_html, smtp::SmtpCreds, DbConn,
    IntoFlash,
};
use askama::Template;
use chacha20poly1305::Nonce;
use chrono::{offset::Utc, DateTime, Duration, NaiveDateTime};
use rand::{prelude::StdRng, RngCore, SeedableRng};
use rocket::{
    form::{Form, Strict},
    http::{Cookie as HttpCookie, CookieJar},
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    categories::{Categories, Category, CtgTrait},
    error::SailsDbError,
    products::*,
    transactions::*,
    users::*,
};

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
fn generate_passwd_reset_link(
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

fn generate_verification_link(dst: &str, aead: &AeadKey) -> anyhow::Result<String> {
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

#[derive(Debug, FromForm, Clone)]
pub struct PartialUserFormOwned {
    pub name: String,
    pub school: String,
    pub description: Option<String>,
}

#[post("/update_user", data = "<info>")]
pub async fn update_user(
    user: UserInfoGuard<Cookie>,
    info: Form<PartialUserFormOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let mut info = info.into_inner();
    info.description = info.description.map(|d| sanitize_html(&d));
    conn.run(move |c| {
        user.info
            .set_description(info.description)
            .set_name(info.name)
            .set_school(info.school)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/user", portal)))
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
            aead.decrypt(&decoded, &timestamp_to_nonce(exp))
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
        generate_passwd_reset_link(user.get_id(), user.get_hashed_passwd(), aead)
            .into_flash(uri!("/"))?,
    )
    .await
    .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/user", portal)))
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

    Ok(Redirect::to(uri!("/user", portal)))
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
    Redirect::to(uri!("/"))
}

#[derive(Template)]
#[template(path = "user/update_user_page.html")]
pub struct UpdateUserPage {
    user: UserInfo,
}

#[get("/update_user_page")]
pub async fn update_user_page(user: UserInfoGuard<Cookie>) -> UpdateUserPage {
    UpdateUserPage { user: user.info }
}

#[derive(Template)]
#[template(path = "user/portal_guest.html")]
pub struct PortalGuestPage {
    user: UserInfo,
    books_operated: Vec<(ProductInfo, Option<Category>)>,
    books_owned: Vec<(ProductInfo, Option<Category>)>,
}

#[derive(Template)]
#[template(path = "user/portal.html")]
pub struct PortalPage {
    user: UserInfo,
    books_operated: Vec<(ProductInfo, Option<Category>)>,
    books_owned: Vec<(ProductInfo, Option<Category>)>,
    orders_placed: Vec<(ProductInfo, TransactionInfo)>,
    orders_received: Vec<(ProductInfo, TransactionInfo)>,
}

#[get("/", rank = 1)]
pub async fn portal_guest(
    _signedin: UserIdGuard<Cookie>,
    user: UserInfoGuard<Param>,
    conn: DbConn,
) -> Result<PortalGuestPage, Flash<Redirect>> {
    let uid = user.info.get_id().to_string();

    let uid_cloned = uid.clone();
    let books_operated = conn
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
                    .chain(
                        ProductFinder::new(c, None)
                            .delegator(&uid_cloned)
                            .search_info()?
                            .into_iter()
                            .map(|x| {
                                let ctg = Categories::find_by_id(c, x.get_category_id()).ok();
                                Ok((x, ctg))
                            }),
                    )
                    .collect()
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let uid_cloned = uid.clone();
    let books_owned = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, Option<Category>)>, SailsDbError> {
                ProductFinder::new(c, None)
                    .owner(&uid_cloned)
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
        .into_flash(uri!("/"))?;
    Ok(PortalGuestPage {
        user: user.info,
        books_operated,
        books_owned,
    })
}

// The flash message is required here because we may get error from update_user
#[get("/", rank = 2)]
pub async fn portal(
    user: UserInfoGuard<Cookie>,
    conn: DbConn,
) -> Result<PortalPage, Flash<Redirect>> {
    let uid = user.info.get_id().to_string();

    let uid_cloned = uid.clone();
    let books_operated = conn
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
                    .chain(
                        ProductFinder::new(c, None)
                            .delegator(&uid_cloned)
                            .search_info()?
                            .into_iter()
                            .map(|x| {
                                let ctg = Categories::find_by_id(c, x.get_category_id()).ok();
                                Ok((x, ctg))
                            }),
                    )
                    .collect()
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let uid_cloned = uid.clone();
    let books_owned = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, Option<Category>)>, SailsDbError> {
                ProductFinder::new(c, None)
                    .owner(&uid_cloned)
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
        .into_flash(uri!("/"))?;

    let uid_cloned = uid.clone();
    let orders_placed = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                TransactionFinder::new(c, None)
                    .buyer(&uid_cloned)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        let product = ProductFinder::new(c, None)
                            .id(x.get_product())
                            .first_info()
                            .unwrap();
                        Ok((product, x))
                    })
                    .collect()
            },
        )
        .await
        .unwrap(); // No error should be tolerated here (database error). 500 is expected

    let orders_received = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                TransactionFinder::new(c, None)
                    .seller(&uid)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        let product = ProductFinder::new(c, None)
                            .id(x.get_product())
                            .first_info()
                            .unwrap();
                        Ok((product, x))
                    })
                    .collect()
            },
        )
        .await
        .unwrap(); // No error should be tolerated here (database error). 500 is expected
    Ok(PortalPage {
        user: user.info,
        orders_placed,
        orders_received,
        books_operated,
        books_owned,
    })
}

#[get("/", rank = 3)]
pub async fn portal_unsigned() -> Redirect {
    Redirect::to(uri!("/user", signin))
}
