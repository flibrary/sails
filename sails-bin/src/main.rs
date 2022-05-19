// A few rule of thumb.
// Use request guard whenever possible to let rocket simplify the boilerplate of the otherwise complicated flow control.
// Don't use flash message everywhere, only use when needed
// If flash message needs to be displayed in place, don't use redirection, just use the Context::msg. And in that case, don't accept flashmessage.
// Handle general database errors by redirecting using flash message to some big pages like `/store`, `/user`. Flash message will only be used up when called.
// All for loops in templates should be able to handle empty vec.

#[macro_use]
extern crate gettext_macros;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket_sync_db_pools;

use crate::i18n::I18n;
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
    http::{uri::Reference, Header, Status},
    request::FlashMessage,
    response::{self, Flash, Redirect},
    shield::Shield,
    Build, Rocket,
};
use rust_embed::RustEmbed;
use sails_db::{
    categories::{Categories, CtgBuilder},
    digicons::{Digicons, DigiconsBuilder},
    tags::{Tags, TagsBuilder},
};
use std::{convert::TryInto, ffi::OsStr, io::Cursor, path::PathBuf};
use structopt::StructOpt;
use telegram_bot::TelegramBot;

init_i18n!("sails", en, zh);

// Following modules may use i18n! or t!, they are required to be called before compile_i18n! per https://github.com/Plume-org/gettext-macros#order-of-the-macros
mod admin;
mod aead;
mod alipay;
mod digicons;
mod guards;
mod i18n;
mod images;
mod messages;
mod orders;
mod recaptcha;
mod root;
mod search;
mod smtp;
mod store;
mod telegram_bot;
mod user;

use crate::{
    alipay::{AlipayAppPrivKey, AlipayClient},
    digicons::DigiconHosting,
    images::ImageHosting,
    recaptcha::ReCaptcha,
    root::RootPasswd,
    smtp::SmtpCreds,
};

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

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    // This macro from `diesel_migrations` defines an `embedded_migrations`
    // module containing a function named `run`. This allows the example to be
    // run and tested without any outside setup of the database.
    embed_migrations!();

    let conn = DbConn::get_one(&rocket).await.expect("database connection");

    let ctg = rocket.state::<CtgBuilder>().cloned();
    let tags = rocket.state::<TagsBuilder>().cloned();
    let digicons = rocket.state::<DigiconsBuilder>().cloned();
    // Initialize the database
    conn.run(|c| {
        // Enforce foreign key relation
        embedded_migrations::run(c).expect("can run migrations");

        c.batch_execute("PRAGMA foreign_keys = OFF;").unwrap();

        // Delete all the categories, digicons, and tags, then we rebuild them.
        Categories::delete_all(c).unwrap();
        Tags::delete_all(c).unwrap();
        Digicons::delete_all(c).unwrap();

        c.batch_execute("PRAGMA foreign_keys = ON;").unwrap();

        if let Some(x) = ctg {
            x.build(c).unwrap()
        }
        if let Some(x) = tags {
            x.build(c).unwrap()
        }
        if let Some(x) = digicons {
            x.build(c).unwrap()
        }
    })
    .await;
    rocket
}

#[derive(Template)]
#[template(path = "index.html")]
struct Index {
    i18n: I18n,
    inner: Msg,
}

#[get("/")]
async fn index<'a>(i18n: I18n, flash: Option<FlashMessage<'_>>) -> Index {
    Index {
        i18n,
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
                    .header(Header::new("Cache-Control", "max-age=31536000"))
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
    about = "The web server for FLibrary, an online learning store"
)]
struct DcompassOpts {
    /// Path to the TOML configuration file.
    #[structopt(short, long, parse(from_os_str))]
    config: PathBuf,
}

compile_i18n!();

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
        .attach(AdHoc::config::<CtgBuilder>())
        .attach(AdHoc::config::<TagsBuilder>())
        .attach(AdHoc::config::<DigiconsBuilder>())
        .attach(AdHoc::config::<RootPasswd>())
        .attach(AdHoc::config::<ReCaptcha>())
        .attach(AdHoc::config::<SmtpCreds>())
        .attach(AdHoc::config::<AeadKey>())
        .attach(AdHoc::config::<ImageHosting>())
        .attach(AdHoc::config::<DigiconHosting>())
        .attach(AdHoc::config::<AlipayAppPrivKey>())
        .attach(AdHoc::config::<AlipayClient>())
        .attach(AdHoc::config::<TelegramBot>())
        .attach(AdHoc::on_ignite("Run database migrations", run_migrations))
        .manage(include_i18n!())
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
            "/store",
            routes![
                store::home_page,
                store::post_prod_page,
                store::update_prod_page,
                store::post_prod_error_page,
                store::update_prod,
                store::prod_page_guest,
                store::prod_page_owned,
                store::prod_page_user,
                store::prod_page_error,
                store::delete_prod,
                store::create_prod,
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
                admin::admin_digicon,
                admin::admin_digicons,
                admin::add_digicon,
                admin::remove_digicon,
                admin::admin_prods,
                admin::admin_metrics,
                admin::verify_prod,
                admin::disable_prod,
                admin::admin_orders,
                admin::refund_order,
                admin::finish_order,
                admin::order_info,
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
                orders::cancel_order,
            ],
        )
        .mount(
            "/images",
            routes![images::upload, images::get, images::get_default],
        )
        .mount("/i18n", routes![i18n::set_lang])
        .mount(
            "/digicons",
            routes![digicons::get, digicons::trace, digicons::trace_unauthorized],
        )
        .register("/", catchers![page404, page422, page500])
}
