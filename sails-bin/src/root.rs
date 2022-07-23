use crate::{
    guards::*,
    utils::{i18n::I18n, recaptcha::ReCaptcha},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    form::Form,
    http::{Cookie, CookieJar, SameSite},
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    enums::UserStatus,
    users::{UserFinder, UserInfo},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RootPasswd {
    #[serde(rename = "root_passwd")]
    passwd: String,
}

impl RootPasswd {
    // There is no need for us to use hash because in this case it is not in the database.
    pub fn verify(&self, passwd: &str) -> bool {
        self.passwd == passwd
    }
}

// Form used for validating root
#[derive(FromForm)]
pub struct Validation {
    password: String,
    #[field(name = "g-recaptcha-response")]
    recaptcha_token: String,
}

#[post("/validate", data = "<info>")]
pub async fn validate(
    jar: &CookieJar<'_>,
    info: Form<Validation>,
    root_passwd: &State<RootPasswd>,
    recaptcha: &State<ReCaptcha>,
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

    if root_passwd.verify(&info.password) {
        let cookie = Cookie::build("root_challenge", "ROOT")
            .secure(true)
            .same_site(SameSite::Strict)
            .finish();
        // Successfully validated, set private cookie.
        jar.add_private(cookie);
        Ok(Redirect::to(uri!("/root", root)))
    } else {
        Err(Flash::error(Redirect::to(uri!("/")), "Incorrect password"))
    }
}

#[derive(Template)]
#[template(path = "root/root_verify.html")]
pub struct RootVerifyPage {
    i18n: I18n,
    recaptcha_key: String,
}

#[get("/root_verify")]
pub async fn root_verify<'a>(i18n: I18n, recaptcha: &State<ReCaptcha>) -> RootVerifyPage {
    RootVerifyPage {
        i18n,
        recaptcha_key: recaptcha.site_key().to_string(),
    }
}

#[derive(Template)]
#[template(path = "root/root.html")]
pub struct RootPage {
    i18n: I18n,
    users: Vec<UserInfo>,
}

// If the user has already been verified, show him the root dashboard
#[get("/", rank = 1)]
pub async fn root(i18n: I18n, _guard: Role<Root>, conn: DbConn) -> Result<RootPage, Redirect> {
    let users = conn.run(|c| UserFinder::list_info(c)).await.unwrap(); // No error should be tolerated here (database error). 500 is expected
    Ok(RootPage { i18n, users })
}

// If the visitor has not yet been verified, redirect them to verification page
#[get("/", rank = 2)]
pub async fn unverified_root() -> Redirect {
    Redirect::to(uri!("/root", root_verify))
}

#[derive(Template)]
#[template(path = "root/user_status.html")]
pub struct UserStatusPage {
    i18n: I18n,
    user: UserInfo,
}

#[get("/user_status?<user_id>")]
pub async fn user_status(
    i18n: I18n,
    _guard: Role<Root>,
    user_id: UserGuard,
    conn: DbConn,
) -> Result<UserStatusPage, Flash<Redirect>> {
    let user = user_id.to_info_param(&conn).await.into_flash(uri!("/"))?;

    Ok(UserStatusPage {
        i18n,
        user: user.info,
    })
}

#[derive(Debug, FromForm, Clone)]
pub struct UserStatusForm {
    pub status: u32,
}

// TODO: Why do we write the user_id into here?
#[post("/user_status?<user_id>", data = "<info>")]
pub async fn update_user_status(
    _guard: Role<Root>,
    conn: DbConn,
    user_id: UserGuard,
    info: Form<UserStatusForm>,
) -> Result<Redirect, Flash<Redirect>> {
    let user = user_id.to_info_param(&conn).await.into_flash(uri!("/"))?;
    let id = user.info.get_id().to_string();
    if let Some(status) = UserStatus::from_bits(info.status) {
        conn.run(move |c| user.info.set_user_status(status).update(c))
            .await
            .into_flash(uri!("/"))?;
        // We cannot get uri macro working
        Ok(Redirect::to(uri!("/root", user_status(id))))
    } else {
        Err(Flash::error(Redirect::to(uri!("/")), "invalid level"))
    }
}

#[get("/delete_user?<user_id>")]
pub async fn delete_user(
    _guard: Role<Root>,
    user_id: UserGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let id = user_id.to_id_param(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| id.id.delete(c)).await.into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/root", root)))
}

#[get("/logout")]
pub async fn logout(jar: &CookieJar<'_>) -> Redirect {
    if let Some(root_challenge) = jar.get_private("root_challenge") {
        jar.remove_private(root_challenge);
    } else {
        // No UID specified, do nothing
    }
    // Redirect back to home
    Redirect::to(uri!("/"))
}
