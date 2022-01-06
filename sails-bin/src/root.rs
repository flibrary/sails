use crate::{guards::*, recaptcha::ReCaptcha, DbConn, IntoFlash};
use askama::Template;
use rocket::{
    form::Form,
    http::{Cookie, CookieJar},
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    enums::Status,
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
        let mut cookie = Cookie::new("root_challenge", "ROOT");
        cookie.set_secure(true);
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
    recaptcha_key: String,
}

#[get("/root_verify")]
pub async fn root_verify<'a>(recaptcha: &State<ReCaptcha>) -> RootVerifyPage {
    RootVerifyPage {
        recaptcha_key: recaptcha.recaptcha_site_key().to_string(),
    }
}

#[derive(Template)]
#[template(path = "root/root.html")]
pub struct RootPage {
    users: Vec<UserInfo>,
}

// If the user has already been verified, show him the root dashboard
#[get("/", rank = 1)]
pub async fn root(_guard: Role<Root>, conn: DbConn) -> Result<RootPage, Redirect> {
    let users = conn.run(|c| UserFinder::list_info(c)).await.unwrap(); // No error should be tolerated here (database error). 500 is expected
    Ok(RootPage { users })
}

// If the visitor has not yet been verified, redirect them to verification page
#[get("/", rank = 2)]
pub async fn unverified_root() -> Redirect {
    Redirect::to(uri!("/root", root_verify))
}

#[get("/promote_user")]
pub async fn promote(
    _guard: Role<Root>,
    info: UserInfoGuard<Param>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        let upgraded = info.info.get_user_status().up();
        info.info.set_user_status(upgraded).update(c).map(|_| ())
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/root", root)))
}

#[get("/downgrade_user")]
pub async fn downgrade(
    _guard: Role<Root>,
    info: UserInfoGuard<Param>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        let downgraded = info.info.get_user_status().down();
        info.info.set_user_status(downgraded).update(c).map(|_| ())
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/root", root)))
}

#[get("/delete_user")]
pub async fn delete_user(
    _guard: Role<Root>,
    id: UserIdGuard<Param>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| id.id.delete(c)).await.into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/root", root)))
}

#[get("/activate_user")]
pub async fn activate_user(
    _guard: Role<Root>,
    info: UserInfoGuard<Param>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| info.info.set_validated(true).update(c))
        .await
        .into_flash(uri!("/"))?;
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
