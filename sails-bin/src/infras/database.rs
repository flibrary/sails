use diesel::connection::SimpleConnection;
use rocket::{Build, Rocket};
use sails_db::{
    categories::{Categories, CtgBuilder},
    tags::{Tags, TagsBuilder},
};

#[database("flibrary")]
pub struct DbConn(diesel::SqliteConnection);

pub async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
    // This macro from `diesel_migrations` defines an `embedded_migrations`
    // module containing a function named `run`. This allows the example to be
    // run and tested without any outside setup of the database.
    embed_migrations!();

    let conn = DbConn::get_one(&rocket).await.expect("database connection");

    let ctg = rocket.state::<CtgBuilder>().cloned();
    let tags = rocket.state::<TagsBuilder>().cloned();
    // Initialize the database
    conn.run(|c| {
        // Enforce foreign key relation
        embedded_migrations::run(c).expect("can run migrations");

        c.batch_execute("PRAGMA foreign_keys = OFF;").unwrap();

        // Delete all the categories, digicons, and tags, then we rebuild them.
        Categories::delete_all(c).unwrap();
        Tags::delete_all(c).unwrap();

        c.batch_execute("PRAGMA foreign_keys = ON;").unwrap();

        if let Some(x) = ctg {
            x.build(c).unwrap()
        }
        if let Some(x) = tags {
            x.build(c).unwrap()
        }
    })
    .await;
    rocket
}
