use crate::{guards::UserGuard, wrap_op, DbConn, Msg};
use askama::Template;
use rocket::{
    form::Form,
    http::{Cookie, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{
    products::{Product, ProductFinder},
    users::{User, Users},
};

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
}

#[get("/signup")]
pub async fn signup<'a>(flash: Option<FlashMessage<'_>>) -> SignUpPage {
    SignUpPage {
        inner: Msg::from_flash(flash),
    }
}

#[post("/create_user", data = "<info>")]
pub async fn create_user(info: Form<UserInfo>, conn: DbConn) -> Result<Redirect, Flash<Redirect>> {
    wrap_op(
        conn.run(move |c| {
            Users::register(c, &info.email, &info.school, &info.phone, &info.password)
        })
        .await,
        uri!("/user", signup),
    )?;
    Ok(Redirect::to(uri!("/user", portal)))
}

#[post("/update_user", data = "<info>")]
pub async fn update_user(info: Form<UserInfo>, conn: DbConn) -> Result<Redirect, Flash<Redirect>> {
    let user = wrap_op(
        User::new(&info.email, &info.school, &info.phone, &info.password),
        uri!("/user", portal),
    )?;
    wrap_op(
        conn.run(move |c| Users::update(c, user)).await,
        uri!("/user", portal),
    )?;

    Ok(Redirect::to(uri!("/user", portal)))
}

#[post("/validate", data = "<info>")]
pub async fn validate(
    jar: &CookieJar<'_>,
    info: Form<Validation>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let user = wrap_op(
        conn.run(move |c| Users::login(c, &info.email, &info.password))
            .await,
        uri!("/user", portal),
    )?;
    let mut cookie = Cookie::new("uid", user);
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
#[template(path = "user/portal.html")]
pub struct PortalPage {
    user: User,
    books: Vec<Product>,
    inner: Msg,
}

// The flash message is required here because we may get error from update_user
#[get("/")]
pub async fn portal(
    flash: Option<FlashMessage<'_>>,
    user: Option<UserGuard>,
    conn: DbConn,
) -> Result<PortalPage, Redirect> {
    if let Some(user) = user.map(|u| u.user) {
        let uid_cloned = user.get_id().to_string();
        // TODO: get rid of this unwrap
        let books = conn
            .run(move |c| ProductFinder::new(c, None).seller(&uid_cloned).search())
            .await
            .unwrap();
        Ok(PortalPage {
            user,
            books,
            inner: Msg::from_flash(flash),
        })
    } else {
        Err(Redirect::to(uri!("/user", signin)))
    }
}
