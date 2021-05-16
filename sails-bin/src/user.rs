use rocket::{
    form::Form,
    http::{Cookie, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_contrib::templates::Template;
use sails_db::{
    products::{Product, ProductFinder},
    users::{User, Users},
};
use serde::Serialize;
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
pub struct UserInfo {
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
    info: Form<UserInfo>,
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

// It's actually quite safe here to use create_or_update, because the check are enforced in the backend.
#[post("/update_user", data = "<info>")]
pub async fn update_user(info: Form<UserInfo>, conn: DbConn) -> Result<Redirect, Flash<Redirect>> {
    let user = User::new(&info.email, &info.school, &info.phone, &info.password)
        .map_err(|e| Flash::error(Redirect::to(uri!("/user", portal)), e.to_string()))?;
    conn.run(move |c| Users::update(c, user))
        .await
        .map_err(|e| Flash::error(Redirect::to(uri!("/user", portal)), e.to_string()))?;

    Ok(Redirect::to(uri!("/user", portal)))
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
pub async fn portal(
    flash: Option<FlashMessage<'_>>,
    jar: &CookieJar<'_>,
    conn: DbConn,
) -> Result<Template, Redirect> {
    #[derive(Serialize)]
    struct UserPortal {
        user: User,
        books: Vec<Product>,
    }

    if let Some(uid) = jar.get_private("uid") {
        let uid = uid.value().to_string();
        // It is only possible that we signed the cookie, which means it is safe to unwrap
        let uid_cloned = uid.clone();
        let user = conn.run(move |c| Users::find_by_id(c, &uid)).await.unwrap();
        let books = conn
            .run(move |c| ProductFinder::new(c, None).seller(&uid_cloned).search())
            .await
            .unwrap();
        let mut cx = Context::new(UserPortal { user, books });
        cx.with_flash(flash);
        Ok(Template::render("portal", cx))
    } else {
        Err(Redirect::to(uri!("/user", signin)))
    }
}
