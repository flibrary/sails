// A few rule of thumb.
// Use request guard whenever possible to let rocket simplify the boilerplate of the otherwise complicated flow control.
// Don't use flash message everywhere, only use when needed
// If flash message needs to be displayed in place, don't use redirection, just use the Context::msg. And in that case, don't accept flashmessage.
// Handle general database errors by redirecting using flash message to some big pages like `/market`, `/user`. Flash message will only be used up when called.
// All for loops in templates should be able to handle empty vec.

mod admin;
mod guards;
mod market;
mod messages;
mod recaptcha;
mod user;

use askama::Template;
use diesel::connection::SimpleConnection;
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
    categories::{Categories, CtgBuilder},
    error::SailsDbError,
};
use std::{convert::TryInto, ffi::OsStr, io::Cursor, path::PathBuf};
use structopt::StructOpt;

use crate::{admin::RootPasswd, recaptcha::ReCaptcha};

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket_sync_db_pools;

// Wraps around the db operation
pub fn wrap_op<T>(
    x: Result<T, SailsDbError>,
    uri: impl TryInto<Reference<'static>>,
) -> Result<T, Flash<Redirect>> {
    x.map_err(|e| Flash::error(Redirect::to(uri), e.to_string()))
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

    pub fn msg(msg: impl ToString) -> Self {
        Self {
            flash: Some(msg.to_string()),
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
    // Initialize the database
    conn.run(|c| {
        // Enforce foreign key relation
        embedded_migrations::run(c).expect("can run migrations");

        c.batch_execute("PRAGMA foreign_keys = OFF;").unwrap();

        // Delete all the categories by default
        Categories::delete_all(c).unwrap();

        c.batch_execute("PRAGMA foreign_keys = ON;").unwrap();

        if let Some(ctg) = ctg {
            ctg.build(c).unwrap()
        } else {
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
                    .sized_body(d.len(), Cursor::new(d))
                    .ok()
            },
        )
    }
}

#[get("/<path..>")]
async fn get_file(path: PathBuf) -> StaticFile {
    StaticFile(path)
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
        .attach(AdHoc::config::<CtgBuilder>())
        .attach(AdHoc::config::<RootPasswd>())
        .attach(AdHoc::config::<ReCaptcha>())
        .attach(AdHoc::on_ignite("Run database migrations", run_migrations))
        .mount("/", routes![index])
        .mount("/static", routes![get_file])
        // Mount user namespace
        .mount(
            "/user",
            routes![
                user::portal,
                user::signin,
                user::validate,
                user::signup,
                user::create_user,
                user::logout,
                user::update_user,
                user::update_user_page,
                user::portal_unsigned
            ],
        )
        .mount(
            "/market",
            routes![
                market::market,
                market::all_books,
                market::categories,
                market::post_book_page,
                market::update_book_page,
                market::post_book_error_page,
                market::update_book,
                market::book_page_guest,
                market::book_page_owned,
                market::book_page_user,
                market::book_page_error,
                market::categories_all,
                market::delete_book,
                market::create_book,
                market::instruction,
            ],
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
            "/admin",
            routes![
                admin::root,
                admin::logout,
                admin::unverified_root,
                admin::validate,
                admin::root_verify,
                admin::promote,
                admin::downgrade,
            ],
        )
        .register("/", catchers![page404, page422, page500])
}
