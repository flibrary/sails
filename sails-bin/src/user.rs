use crate::{
    guards::{UserIdGuard, UserInfoGuard},
    wrap_op, DbConn, Msg,
};
use askama::Template;
use check_if_email_exists::{check_email, CheckEmailInput, Reachable};
use rocket::{
    form::{Form, Strict},
    http::{Cookie, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{
    categories::{Categories, Category, CtgTrait},
    error::SailsDbError,
    products::*,
    users::*,
};

// Form used for validating an user
#[derive(FromForm)]
pub struct Validation {
    email: String,
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
pub async fn create_user(
    info: Form<Strict<UserFormOwned>>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    // TODO: Get rid of check email
    let res = check_email(&CheckEmailInput::new(vec![info.id.clone()])).await;
    // If the server is invalid, then the output will be `Reachable::Invalid`
    if (res.get(0).unwrap().is_reachable == Reachable::Safe)
        || (res.get(0).unwrap().is_reachable == Reachable::Unknown)
    {
        wrap_op(
            conn.run(move |c| info.to_ref()?.create(c)).await,
            uri!("/user", signup),
        )?;
        Ok(Redirect::to(uri!("/user", portal)))
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/user", portal)),
            "your email address is considered unreachable",
        ))
    }
}

#[post("/update_user", data = "<info>")]
pub async fn update_user(
    user: UserIdGuard,
    info: Form<UserFormOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    if user.id.get_id() == info.id {
        wrap_op(
            conn.run(move |c| info.to_ref()?.update(c)).await,
            uri!("/user", portal),
        )?;

        Ok(Redirect::to(uri!("/user", portal)))
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/user", portal)),
            "not authorized to update",
        ))
    }
}

#[post("/validate", data = "<info>")]
pub async fn validate(
    jar: &CookieJar<'_>,
    info: Form<Validation>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let user = wrap_op(
        conn.run(move |c| UserId::login(c, &info.email, &info.password))
            .await,
        uri!("/user", portal),
    )?;
    let mut cookie = Cookie::new("uid", user.get_id().to_string());
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
#[template(path = "user/update_user_page.html")]
pub struct UpdateUserPage {
    user: UserInfo,
}

#[get("/update_user_page")]
pub async fn update_user_page(user: UserInfoGuard) -> UpdateUserPage {
    UpdateUserPage { user: user.info }
}

#[derive(Template)]
#[template(path = "user/portal.html")]
pub struct PortalPage {
    user: UserInfo,
    books: Vec<(ProductInfo, Option<Category>)>,
    inner: Msg,
}

// The flash message is required here because we may get error from update_user
#[get("/")]
pub async fn portal(
    flash: Option<FlashMessage<'_>>,
    user: Option<UserInfoGuard>,
    conn: DbConn,
) -> Result<PortalPage, Redirect> {
    if let Some(user) = user.map(|u| u.info) {
        let uid_cloned = user.get_id().to_string();
        let books = conn
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
                        .collect()
                },
            )
            .await
            .unwrap(); // No error should be tolerated here (database error). 500 is expected
        Ok(PortalPage {
            user,
            books,
            inner: Msg::from_flash(flash),
        })
    } else {
        Err(Redirect::to(uri!("/user", signin)))
    }
}
