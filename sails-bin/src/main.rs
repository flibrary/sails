// A few rule of thumb.
// Use request guard whenever possible to let rocket simplify the boilerplate of the otherwise complicated flow control.
// Don't use flash message everywhere, only use when needed
// If flash message needs to be displayed in place, don't use redirection, just use the Context::msg. And in that case, don't accept flashmessage.
// Handle general database errors by redirecting using flash message to some big pages like `/market`, `/user`. Flash message will only be used up when called.
// All for loops in templates should be able to handle empty vec.

mod admin;
mod aead;
mod alipay;
mod guards;
mod images;
mod market;
mod messages;
mod orders;
mod recaptcha;
mod root;
mod search;
mod smtp;
mod store;
mod user;

use aead::AeadKey;
use ammonia::Builder;
use askama::Template;
use diesel::connection::SimpleConnection;
use once_cell::sync::Lazy;
use rocket::{
    fairing::AdHoc,
    figment::{
        providers::{Format, Toml},
        Figment,
    },
    http::{uri::Reference, Status},
    request::FlashMessage,
    response::{self, Flash, Redirect},
    shield::Shield,
    Build, Rocket,
};
use rust_embed::RustEmbed;
use sails_db::{
    categories::{Categories, CtgBuilder, Value},
    tags::{Tags, TagsBuilder},
};
use serde::{Deserialize, Serialize};
use std::{convert::TryInto, ffi::OsStr, io::Cursor, path::PathBuf};
use structopt::StructOpt;

use crate::{
    alipay::{AlipayAppPrivKey, AlipayClient},
    images::ImageHosting,
    recaptcha::ReCaptcha,
    root::RootPasswd,
    smtp::SmtpCreds,
};

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket_sync_db_pools;

pub fn sanitize_html(html: &str) -> String {
    SANITIZER.clean(html).to_string()
}

// Comrak options. We selectively enabled a few GFM standards.
static SANITIZER: Lazy<Builder> = Lazy::new(|| {
    let mut builder = ammonia::Builder::default();
    // DANGEROUS: Style attributes are dangerous
    builder
        .add_tag_attributes("img", &["style"])
        .add_tag_attributes("span", &["style"])
        .add_tags(&["font"])
        .add_tag_attributes("font", &["color"])
        .add_generic_attributes(&["align"]);
    builder
});

pub trait IntoFlash<T> {
    fn into_flash(self, uri: impl TryInto<Reference<'static>>) -> Result<T, Flash<Redirect>>;
}

impl<T, E> IntoFlash<T> for Result<T, E>
where
    E: std::fmt::Display,
{
    fn into_flash(self, uri: impl TryInto<Reference<'static>>) -> Result<T, Flash<Redirect>> {
        self.map_err(|e| Flash::error(Redirect::to(uri), e.to_string()))
    }
}

#[database("flibrary")]
pub struct DbConn(diesel::SqliteConnection);

// A short hand message <-> flash conversion
pub struct Msg {
    flash: Option<String>,
}

impl Msg {
    // Construct a message from a flash message
    pub fn from_flash(flash: Option<FlashMessage<'_>>) -> Self {
        Self {
            flash: flash.map(|f| format!("{}: {}", f.kind(), f.message())),
        }
    }

    pub fn new(payload: impl ToString) -> Self {
        Self {
            flash: Some(payload.to_string()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
struct CtgFramework {
    market: CtgBuilder,
    store: CtgBuilder,
}

impl CtgFramework {
    fn into_builder(self) -> CtgBuilder {
        CtgBuilder::new(maplit::hashmap! {
            "书本市场".into() => Value::SubCategory(self.market.inner()),
            "Store 在线商店".into() => Value::SubCategory(self.store.inner()),
        })
    }
}

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    // This macro from `diesel_migrations` defines an `embedded_migrations`
    // module containing a function named `run`. This allows the example to be
    // run and tested without any outside setup of the database.
    embed_migrations!();

    let conn = DbConn::get_one(&rocket).await.expect("database connection");

    let ctg = rocket.state::<CtgFramework>().cloned();
    let tags = rocket.state::<TagsBuilder>().cloned();
    // Initialize the database
    conn.run(|c| {
        // Enforce foreign key relation
        embedded_migrations::run(c).expect("can run migrations");

        c.batch_execute("PRAGMA foreign_keys = OFF;").unwrap();

        // Delete all the categories and tags, then we rebuild them.
        Categories::delete_all(c).unwrap();
        Tags::delete_all(c).unwrap();

        c.batch_execute("PRAGMA foreign_keys = ON;").unwrap();

        if let Some(x) = ctg {
            x.into_builder().build(c).unwrap()
        }
        if let Some(x) = tags {
            x.build(c).unwrap()
        }
    })
    .await;
    rocket
}

#[derive(Template)]
#[template(path = "index.html")]
struct Index {
    inner: Msg,
}

#[get("/")]
async fn index<'a>(flash: Option<FlashMessage<'_>>) -> Index {
    Index {
        inner: Msg::from_flash(flash),
    }
}

#[derive(RustEmbed)]
#[folder = "static/"]
struct Asset;

struct StaticFile(PathBuf);

impl<'r, 'o: 'r> rocket::response::Responder<'r, 'o> for StaticFile {
    fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'o> {
        let filename = self.0.display().to_string();
        Asset::get(&filename).map_or_else(
            || Err(Status::NotFound),
            |d| {
                let ext = self
                    .0
                    .as_path()
                    .extension()
                    .and_then(OsStr::to_str)
                    .ok_or_else(|| Status::new(400))?;
                let content_type = rocket::http::ContentType::from_extension(ext)
                    .ok_or_else(|| Status::new(400))?;
                response::Response::build()
                    .header(content_type)
                    .sized_body(d.data.len(), Cursor::new(d.data))
                    .ok()
            },
        )
    }
}

#[get("/<path..>")]
async fn get_file(path: PathBuf) -> StaticFile {
    StaticFile(path)
}

#[get("/favicon.ico")]
async fn get_icon() -> Redirect {
    Redirect::to(uri!("/static/favicon.ico"))
}

#[catch(404)]
async fn page404<'a>() -> Redirect {
    Redirect::to("/static/404.html")
}

#[catch(422)]
async fn page422<'a>() -> Redirect {
    Redirect::to("/static/422.html")
}

#[catch(500)]
async fn page500<'a>() -> Redirect {
    Redirect::to("/static/500.html")
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "sails-bin",
    about = "The web server for FLibrary, an online second-hand book market"
)]
struct DcompassOpts {
    /// Path to the TOML configuration file.
    #[structopt(short, long, parse(from_os_str))]
    config: PathBuf,
}

#[launch]
fn rocket() -> Rocket<Build> {
    let args: DcompassOpts = DcompassOpts::from_args();

    // This helps us manage run-time Rocket.toml easily
    let figment = Figment::from(rocket::Config::default()).merge(Toml::file(args.config).nested());

    // According to the documentation, this will not read `Rocket.toml`
    // only Rocket::build reads it.
    rocket::custom(figment)
        .attach(DbConn::fairing())
        .attach(Shield::new())
        .attach(AdHoc::config::<CtgFramework>())
        .attach(AdHoc::config::<TagsBuilder>())
        .attach(AdHoc::config::<RootPasswd>())
        .attach(AdHoc::config::<ReCaptcha>())
        .attach(AdHoc::config::<SmtpCreds>())
        .attach(AdHoc::config::<AeadKey>())
        .attach(AdHoc::config::<ImageHosting>())
        .attach(AdHoc::config::<AlipayAppPrivKey>())
        .attach(AdHoc::config::<AlipayClient>())
        .attach(AdHoc::on_ignite("Run database migrations", run_migrations))
        .mount("/", routes![index, get_icon])
        .mount("/static", routes![get_file])
        // Mount user namespace
        .mount(
            "/user",
            routes![
                user::portal,
                user::portal_guest,
                user::change_passwd_page,
                user::change_passwd_post,
                user::signin,
                user::validate,
                user::signup,
                user::create_user,
                user::logout,
                user::update_user,
                user::update_user_page,
                user::portal_unsigned,
                user::activate_user,
                user::signup_instruction,
                user::email_verified,
                user::reset_passwd_page,
                user::reset_passwd_post,
                user::reset_passwd_now,
                user::reset_passwd_instruction
            ],
        )
        .mount(
            "/market",
            routes![
                market::market,
                market::explore_page,
                market::post_book_page,
                market::post_book_admin_page,
                market::post_book_interim,
                market::delegate_book_page,
                market::delegate_book,
                market::delegate_book_error_page,
                market::update_book_page,
                market::update_book_admin_page,
                market::post_book_error_page,
                market::update_book,
                market::book_page_guest,
                market::book_page_owned,
                market::book_page_user,
                market::book_page_error,
                market::delete_book,
                market::create_book,
                market::instruction,
                market::deposit_info,
                market::deposit_progress,
            ],
        )
        .mount(
            "/search",
            routes![search::categories, search::categories_all],
        )
        .mount(
            "/messages",
            routes![
                messages::portal,
                messages::chat,
                messages::chat_error,
                messages::send
            ],
        )
        .mount(
            "/root",
            routes![
                root::root,
                root::logout,
                root::unverified_root,
                root::validate,
                root::root_verify,
                root::user_status,
                root::update_user_status,
                root::delete_user,
                root::activate_user,
            ],
        )
        .mount(
            "/admin",
            routes![
                admin::admin,
                admin::admin_tag,
                admin::admin_tags,
                admin::add_tag,
                admin::remove_tag,
                admin::admin_books,
                admin::admin_metrics,
                admin::verify_book,
                admin::disable_book,
                admin::normalize_book,
                admin::admin_orders,
                admin::refund_order,
                admin::finish_order,
            ],
        )
        .mount(
            "/orders",
            routes![
                orders::purchase,
                orders::checkout,
                orders::progress,
                orders::order_info_buyer,
                orders::order_info_seller,
                orders::user_cancel_order,
            ],
        )
        .mount(
            "/images",
            routes![images::upload, images::get, images::get_default],
        )
        .mount("/store", routes![store::home_page])
        .register("/", catchers![page404, page422, page500])
}
