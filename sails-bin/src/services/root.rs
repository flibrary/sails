use crate::{
    infras::{guards::*, recaptcha::ReCaptcha},
    pages::root::*,
    DbConn, IntoFlash,
};
use rocket::{
    form::Form,
    http::{Cookie, CookieJar, SameSite},
    response::{Flash, Redirect},
    State,
};
use sails_db::enums::UserStatus;
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
