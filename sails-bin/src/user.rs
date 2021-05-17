use rocket::{
    form::Form,
    http::{Cookie, CookieJar},
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::{FlashMessage, FromRequest},
    response::{Flash, Redirect},
};
use rocket_contrib::templates::Template;
use sails_db::{
    products::{Product, ProductFinder},
    users::{User, Users},
};
use serde::Serialize;

use crate::{wrap_op, Context, DbConn};

const NAMESPACE: &str = "/user";

// This request guard gets us an user if the user ID is specified and validated
pub struct UserWrap(User);

impl UserWrap {
    pub fn inner(self) -> User {
        self.0
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserWrap {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let uid = request
            .cookies()
            .get_private("uid")
            .map(|cookie| cookie.value().to_string());
        if let Some(uid) = uid {
            let uid_inner = uid.clone();
            db.run(move |c| Users::find_by_id(c, &uid_inner))
                .await
                .map(UserWrap)
                .ok()
                .or_forward(())
        } else {
            Outcome::Forward(())
        }
    }
}

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
    Template::render("signin", Context::from_flash(flash))
}

#[get("/signup")]
pub async fn signup<'a>(flash: Option<FlashMessage<'_>>) -> Template {
    Template::render("signup", Context::from_flash(flash))
}

#[post("/create_user", data = "<info>")]
pub async fn create_user(
    jar: &CookieJar<'_>,
    info: Form<UserInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let user = wrap_op(
        conn.run(move |c| {
            Users::register(c, &info.email, &info.school, &info.phone, &info.password)
        })
        .await,
        uri!("/user", signup),
    )?;
    let mut cookie = Cookie::new("uid", user);
    cookie.set_secure(true);
    // Successfully validated, set private cookie.
    jar.add_private(cookie);
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
        NAMESPACE,
    )?;
    // Successfully validated, set private cookie.
    jar.add_private(Cookie::new("uid", user));
    Ok(Redirect::to(NAMESPACE))
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

// The flash message is required here because we may get error from update_user
#[get("/")]
pub async fn portal(
    flash: Option<FlashMessage<'_>>,
    user: Option<UserWrap>,
    conn: DbConn,
) -> Result<Template, Redirect> {
    #[derive(Serialize)]
    struct UserPortal {
        user: User,
        books: Vec<Product>,
    }

    if let Some(user) = user.map(|u| u.inner()) {
        let uid_cloned = user.get_id().to_string();
        // TODO: get rid of this unwrap
        let books = conn
            .run(move |c| ProductFinder::new(c, None).seller(&uid_cloned).search())
            .await
            .unwrap();
        Ok(Template::render(
            "portal",
            Context::new(UserPortal { user, books }, flash),
        ))
    } else {
        Err(Redirect::to(uri!("/user", signin)))
    }
}
