// A few rule of thumb.
// Use request guard whenever possible to let rocket simplify the boilerplate of the otherwise complicated flow control.
// Don't use flash message everywhere, only use when needed
// If flash message needs to be displayed in place, don't use redirection, just use the Context::msg. And in that case, don't accept flashmessage.
// Handle general database errors by redirecting using flash message to some big pages like `/market`, `/user`. Flash message will only be used up when called.
// All for loops in templates should be able to handle empty vec.

mod market;
mod user;

use std::convert::TryInto;

use rocket_contrib::helmet::SpaceHelmet;
use sails_db::{categories::Categories, error::SailsDbError};
use serde::Serialize;
use serde_json::{json, Value};
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket_contrib;

// Wraps around the db operation
pub fn wrap_op<T>(
    x: Result<T, SailsDbError>,
    uri: impl TryInto<Uri<'static>>,
) -> Result<T, Flash<Redirect>> {
    x.map_err(|e| Flash::error(Redirect::to(uri), e.to_string()))
}

#[database("sqlite_database")]
pub struct DbConn(diesel::SqliteConnection);

#[derive(Serialize)]
pub struct Context<T> {
    content: T,
    flash: Option<String>,
}

impl Context<Value> {
    // Construct a context from a flash message and an empty json set
    pub fn from_flash(flash: Option<FlashMessage<'_>>) -> Self {
        Context::new(json!({}), flash)
    }

    // Construct a context from a custom error message and an empty json set
    pub fn err(err: impl ToString) -> Self {
        Context::new_raw(json!({}), Some(err))
    }
}

impl<T: Serialize> Context<T> {
    // Add a flash message to an existing context
    pub fn with_flash(&mut self, flash: Option<FlashMessage<'_>>) {
        self.flash = flash.map(|f| format!("{}: {}", f.kind(), f.message()))
    }

    // Create a new context
    pub fn new_raw(content: T, flash: Option<impl ToString>) -> Context<T> {
        Self {
            content,
            flash: flash.map(|f| f.to_string()),
        }
    }

    pub fn new(content: T, flash: Option<FlashMessage<'_>>) -> Context<T> {
        Self::new_raw(
            content,
            flash.map(|f| format!("{}: {}", f.kind(), f.message())),
        )
    }

    // Create a new context with an existing content and no flash
    pub fn from_content(content: T) -> Self {
        Self {
            content,
            flash: None,
        }
    }
}

use diesel::connection::SimpleConnection;
use rocket::{
    fairing::AdHoc,
    http::uri::Uri,
    request::FlashMessage,
    response::{content, Flash, Redirect},
    Build, Rocket,
};

use rocket_contrib::{
    serve::{crate_relative, StaticFiles},
    templates::Template,
};

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    // This macro from `diesel_migrations` defines an `embedded_migrations`
    // module containing a function named `run`. This allows the example to be
    // run and tested without any outside setup of the database.
    embed_migrations!();

    let conn = DbConn::get_one(&rocket).await.expect("database connection");
    conn.run(|c| {
        // Enforce foreign key relation
        c.batch_execute("PRAGMA foreign_keys = ON;").unwrap();
        embedded_migrations::run(c).expect("can run migrations");

        if Categories::list(c).unwrap().is_empty() {
            // The categories table is empty, create new one by default.
            // If there is an error, ignore it
            let _ = Categories::create(c, "High School");

            let _ = Categories::create(c, "Economics");
            let _ = Categories::insert(c, "Economics", "High School");

            let _ = Categories::create(c, "Physics");
            let _ = Categories::insert(c, "Physics", "High School");

            let _ = Categories::create(c, "English");
            let _ = Categories::insert(c, "English", "High School");

            let _ = Categories::create(c, "Chemistry");
            let _ = Categories::insert(c, "Chemistry", "High School");

            let _ = Categories::create(c, "Biology");
            let _ = Categories::insert(c, "Biology", "High School");

            let _ = Categories::create(c, "Business");
            let _ = Categories::insert(c, "Business", "High School");
        } else {
            // Do nothing because else UUID of the category changes, which breaks the product references
        }
    })
    .await;

    rocket
}

#[get("/")]
async fn index<'a>(flash: Option<FlashMessage<'_>>) -> Template {
    Template::render("index", Context::from_flash(flash))
}

#[catch(404)]
async fn page404<'a>() -> content::Html<&'a str> {
    content::Html(include_str!("../static/404.html"))
}

#[catch(422)]
async fn page422<'a>() -> content::Html<&'a str> {
    content::Html(include_str!("../static/422.html"))
}

#[catch(500)]
async fn page500<'a>() -> content::Html<&'a str> {
    content::Html(include_str!("../static/500.html"))
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(DbConn::fairing())
        .attach(Template::fairing())
        .attach(SpaceHelmet::default())
        .attach(AdHoc::on_ignite("Run database migrations", run_migrations))
        .mount("/", StaticFiles::from(crate_relative!("static")))
        .mount("/", routes![index])
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
                user::update_user
            ],
        )
        .mount(
            "/market",
            routes![
                market::market,
                market::all_products,
                market::categories,
                market::post_book,
                market::update_book,
                market::book_page,
                market::delete_book
            ],
        )
        .register("/", catchers![page404, page422, page500])
}
