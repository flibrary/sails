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

use askama::Template;
use diesel::connection::SimpleConnection;
use rocket::{
    fairing::AdHoc,
    figment::{
        providers::{Format, Toml},
        Figment,
    },
    request::FlashMessage,
    response::Redirect,
    shield::Shield,
    Build, Rocket,
};
use sails_db::{
    categories::{Categories, CtgBuilder},
    tags::{Tags, TagsBuilder},
};
use std::path::PathBuf;
use structopt::StructOpt;
use utils::{i18n::I18n, misc::*};

init_i18n!("sails", en, zh);

// Following modules may use i18n! or t!, they are required to be called before compile_i18n! per https://github.com/Plume-org/gettext-macros#order-of-the-macros
mod admin;
mod digicons;
mod guards;
mod messages;
mod orders;
mod root;
mod search;
mod store;
mod user;
mod utils;

#[database("flibrary")]
pub struct DbConn(diesel::SqliteConnection);

async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {
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

#[derive(Template)]
#[template(path = "joinus.html")]
struct JoinUs {
    i18n: I18n,
}

#[get("/joinus")]
async fn joinus(i18n: I18n) -> JoinUs {
    JoinUs { i18n }
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
    use crate::{digicons::DigiconHosting, root::RootPasswd, utils::oidc::OIDCClient};
    use orders::PaypalAuth;
    use utils::{
        aead::AeadKey,
        alipay::{AlipayAppPrivKey, AlipayClient},
        images::ImageHosting,
        misc::*,
        recaptcha::ReCaptcha,
        smtp::SmtpCreds,
        telegram_bot::TelegramBot,
    };

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
        .attach(AdHoc::config::<RootPasswd>())
        .attach(create_fairing::<ReCaptcha>("recaptcha"))
        .attach(create_fairing::<SmtpCreds>("mailbox"))
        .attach(create_fairing::<AeadKey>("encryption"))
        .attach(create_fairing::<ImageHosting>("images"))
        .attach(create_fairing::<DigiconHosting>("digicons"))
        .attach(create_fairing::<AlipayAppPrivKey>("alipay"))
        .attach(create_fairing::<AlipayClient>("alipay"))
        .attach(create_fairing::<PaypalAuth>("paypal"))
        .attach(create_fairing::<TelegramBot>("telegram"))
        .attach(OIDCClient::fairing())
        .attach(AdHoc::on_ignite("Run database migrations", run_migrations))
        .manage(include_i18n!())
        .mount("/", routes![index, get_icon, joinus])
        .mount("/static", routes![get_file])
        // Mount user namespace
        .mount(
            "/user",
            routes![
                user::signin,
                user::signin_callback,
                user::portal,
                user::portal_guest,
                user::logout,
                user::logout_fallback,
                user::update_user,
                user::update_user_page,
                user::portal_unsigned,
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
                orders::order_info_seller,
                orders::progress_alipay,
                orders::order_info_alipay,
                orders::cancel_order_alipay,
                orders::progress_paypal,
                orders::create_paypal_order,
                orders::capture_paypal_order,
                orders::order_info_paypal,
                orders::cancel_order_paypal,
            ],
        )
        .mount(
            "/images",
            routes![
                utils::images::upload,
                utils::images::get,
                utils::images::get_default
            ],
        )
        .mount("/i18n", routes![utils::i18n::set_lang])
        .mount(
            "/digicons",
            routes![
                digicons::get,
                digicons::trace,
                digicons::trace_unauthorized,
                digicons::delete,
                digicons::upload,
                digicons::update_digicon,
                digicons::create_digicon,
                digicons::create_digicon_page,
                digicons::all_digicons,
                digicons::digicon_page,
                digicons::digicons_center_not_permitted,
                digicons::add_digicon_mapping,
                digicons::remove_digicon_mapping,
            ],
        )
        .register("/", catchers![page404, page422, page500])
}
