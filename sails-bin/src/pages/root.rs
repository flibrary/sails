use crate::{
    infras::{guards::*, i18n::I18n, recaptcha::ReCaptcha},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::users::{UserFinder, UserInfo};

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
