use crate::{
    guards::{RootGuard, UserInfoParamGuard},
    wrap_op, DbConn, Msg,
};
use askama::Template;
use rocket::{
    form::Form,
    http::{Cookie, CookieJar},
    request::FlashMessage,
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
}

#[post("/validate", data = "<info>")]
pub async fn validate(
    jar: &CookieJar<'_>,
    info: Form<Validation>,
    root_passwd: &State<RootPasswd>,
) -> Result<Redirect, Flash<Redirect>> {
    if root_passwd.verify(&info.password) {
        let mut cookie = Cookie::new("root_challenge", "ROOT");
        cookie.set_secure(true);
        // Successfully validated, set private cookie.
        jar.add_private(cookie);
        Ok(Redirect::to(uri!("/admin", root)))
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/admin", root)),
            "Incorrect password",
        ))
    }
}

#[derive(Template)]
#[template(path = "admin/root_verify.html")]
pub struct RootVerifyPage {
    inner: Msg,
}

#[get("/root_verify")]
pub async fn root_verify<'a>(flash: Option<FlashMessage<'_>>) -> RootVerifyPage {
    RootVerifyPage {
        inner: Msg::from_flash(flash),
    }
}

#[derive(Template)]
#[template(path = "admin/root.html")]
pub struct RootPage {
    inner: Msg,
    users: Vec<UserInfo>,
}

// If the user has already been verified, show him the root dashboard
#[get("/root", rank = 1)]
pub async fn root(
    flash: Option<FlashMessage<'_>>,
    _guard: RootGuard,
    conn: DbConn,
) -> Result<RootPage, Redirect> {
    let users = conn.run(|c| UserFinder::list_info(c)).await.unwrap(); // No error should be tolerated here (database error). 500 is expected
    Ok(RootPage {
        users,
        inner: Msg::from_flash(flash),
    })
}

// If the visitor has not yet been verified, redirect them to verification page
#[get("/root", rank = 2)]
pub async fn unverified_root() -> Redirect {
    Redirect::to(uri!("/admin", root_verify))
}

#[get("/promote_user")]
pub async fn promote(
    _guard: RootGuard,
    info: UserInfoParamGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    wrap_op(
        conn.run(|c| match *info.info.get_user_status() {
            UserStatus::Disabled => info
                .info
                .set_user_status(UserStatus::Normal)
                .update(c)
                .map(|_| ()),
            UserStatus::Normal => info
                .info
                .set_user_status(UserStatus::Admin)
                .update(c)
                .map(|_| ()),
            UserStatus::Admin => Ok(()),
        })
        .await,
        uri!("/admin", root),
    )?;
    Ok(Redirect::to(uri!("/admin", root)))
}

#[get("/downgrade_user")]
pub async fn downgrade(
    _guard: RootGuard,
    info: UserInfoParamGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    wrap_op(
        conn.run(|c| match *info.info.get_user_status() {
            UserStatus::Disabled => Ok(()),
            UserStatus::Normal => info
                .info
                .set_user_status(UserStatus::Disabled)
                .update(c)
                .map(|_| ()),
            UserStatus::Admin => info
                .info
                .set_user_status(UserStatus::Normal)
                .update(c)
                .map(|_| ()),
        })
        .await,
        uri!("/admin", root),
    )?;
    Ok(Redirect::to(uri!("/admin", root)))
}
