use rocket::{
    form::Form,
    http::{Cookie, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_contrib::templates::Template;
use sails_db::users::Users;
use serde_json::json;

use crate::{Context, DbConn};

const NAMESPACE: &'static str = "/user";

// Form used for validating an user
#[derive(FromForm)]
pub struct Validation {
    email: String,
    password: String,
}

// Form used for registration
#[derive(FromForm)]
pub struct Registration {
    email: String,
    school: String,
    phone: String,
    password: String,
}

// This would be mounted under namespace `user` and eventually become `/user/signin`
#[get("/signin")]
pub async fn signin<'a>(flash: Option<FlashMessage<'_>>) -> Template {
    let mut cx = Context::new(json!({}));
    cx.with_flash(flash);
    Template::render("signin", cx)
}

#[get("/signup")]
pub async fn signup<'a>(flash: Option<FlashMessage<'_>>) -> Template {
    let mut cx = Context::new(json!({}));
    cx.with_flash(flash);
    Template::render("signup", cx)
}

#[post("/create_user", data = "<info>")]
pub async fn create_user(
    jar: &CookieJar<'_>,
    info: Form<Registration>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    match conn
        .run(move |c| Users::register(c, &info.email, &info.school, &info.phone, &info.password))
        .await
    {
        Ok(user) => {
            // Successfully validated, set private cookie.
            jar.add_private(Cookie::new("uid", user));
            Ok(Redirect::to(uri!("/user", portal)))
        }
        Err(e) => Err(Flash::error(
            Redirect::to(uri!("/user", signup)),
            e.to_string(),
        )),
    }
}

#[post("/validate", data = "<info>")]
pub async fn validate(
    jar: &CookieJar<'_>,
    info: Form<Validation>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    match conn
        .run(move |c| Users::login(c, &info.email, &info.password))
        .await
    {
        Ok(user) => {
            // Successfully validated, set private cookie.
            jar.add_private(Cookie::new("uid", user));
            Ok(Redirect::to(NAMESPACE))
        }
        Err(e) => Err(Flash::error(Redirect::to(NAMESPACE), e.to_string())),
    }
}

#[get("/logout")]
pub async fn logout(jar: &CookieJar<'_>) -> Redirect {
    if let Some(uid) = jar.get_private("uid") {
        jar.remove_private(uid);
    } else {
    }
    // Redirect back to home
    Redirect::to("/")
}

#[get("/")]
pub async fn portal(jar: &CookieJar<'_>, conn: DbConn) -> Result<Template, Redirect> {
    if let Some(uid) = jar.get_private("uid") {
        let uid = uid.value().to_string();
        let cx = Context::new(conn.run(move |c| Users::find_by_id(c, &uid)).await.unwrap());
        Ok(Template::render("portal", cx))
    } else {
        Err(Redirect::to(uri!("/user", signin)))
    }
}
