mod market;
mod user;

use rocket_contrib::helmet::SpaceHelmet;
use sails_db::categories::Categories;
use serde::Serialize;
use serde_json::json;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate rocket_contrib;

#[database("sqlite_database")]
pub struct DbConn(diesel::SqliteConnection);

#[derive(Serialize)]
pub struct Context<T> {
    content: T,
    flash: Option<String>,
}

impl<T: Serialize> Context<T> {
    pub fn with_flash(&mut self, flash: Option<FlashMessage<'_>>) {
        self.flash = flash.map(|f| format!("{}: {}", f.kind(), f.message()))
    }

    pub fn msg(content: T, msg: impl ToString) -> Self {
        Self {
            content,
            flash: Some(msg.to_string()),
        }
    }

    pub fn new(content: T) -> Self {
        Self {
            content,
            flash: None,
        }
    }
}

use diesel::connection::SimpleConnection;
use rocket::{fairing::AdHoc, request::FlashMessage, response::content, Build, Rocket};

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
async fn index<'a>() -> Template {
    Template::render("index", json!({}))
}

#[catch(404)]
async fn page404<'a>() -> content::Html<&'a str> {
    content::Html(include_str!("../static/404.html"))
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
                market::create_book,
                market::book_page
            ],
        )
        .register("/", catchers![page404])
}
