// A few rules of thumb.
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

pub use infras::{basics::*, database::DbConn};
use rocket::{
    fairing::AdHoc,
    figment::{
        providers::{Format, Toml},
        Figment,
    },
    shield::Shield,
    Build, Rocket,
};
use std::path::PathBuf;
use structopt::StructOpt;

init_i18n!("sails", en, zh);

// Following modules may use i18n! or t!, they are required to be called before compile_i18n! per https://github.com/Plume-org/gettext-macros#order-of-the-macros
// mod digicons;
mod infras;
// mod messages;
// mod orders;
mod pages;
// mod root;
// mod search;
mod services;
// mod store;
// mod user;

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
    use crate::{
        infras::{digicons::DigiconHosting, oidc::OIDCClient},
        services::root::RootPasswd,
    };
    use infras::{
        aead::AeadKey,
        alipay::{AlipayAppPrivKey, AlipayClient},
        basics::create_fairing,
        images::ImageHosting,
        recaptcha::ReCaptcha,
        smtp::SmtpCreds,
        tg_bot::TelegramBot,
    };
    use sails_db::{categories::CtgBuilder, tags::TagsBuilder};
    use services::orders::PaypalAuth;

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
        .attach(DigiconHosting::fairing())
        .attach(create_fairing::<AlipayAppPrivKey>("alipay"))
        .attach(create_fairing::<AlipayClient>("alipay"))
        .attach(create_fairing::<PaypalAuth>("paypal"))
        .attach(create_fairing::<TelegramBot>("telegram"))
        .attach(OIDCClient::fairing())
        .attach(AdHoc::on_ignite(
            "Run database migrations",
            infras::database::run_migrations,
        ))
        .manage(include_i18n!())
        .mount(
            "/",
            routes![
                pages::basics::index,
                pages::basics::get_icon,
                pages::basics::joinus
            ],
        )
        .mount("/static", routes![pages::basics::get_file])
        // Mount user namespace
        .mount(
            "/user",
            routes![
                pages::users::portal,
                pages::users::portal_guest,
                pages::users::update_user_page,
                pages::users::portal_unsigned,
                services::users::signin,
                services::users::signin_callback,
                services::users::logout,
                services::users::logout_fallback,
                services::users::update_user,
            ],
        )
        .mount(
            "/store",
            routes![
                pages::store::home_page,
                pages::store::prod_page_guest,
                pages::store::prod_page_owned,
                pages::store::prod_page_user,
                pages::store::prod_page_error,
                pages::store::post_prod_page,
                pages::store::update_prod_page,
                pages::store::post_prod_error_page,
                services::prods::update_prod,
                services::prods::delete_prod,
                services::prods::create_prod,
            ],
        )
        .mount(
            "/search",
            routes![pages::search::categories, pages::search::categories_all],
        )
        .mount(
            "/messages",
            routes![
                pages::msgs::portal,
                pages::msgs::chat,
                pages::msgs::chat_error,
                services::msgs::send
            ],
        )
        .mount(
            "/root",
            routes![
                pages::root::root,
                pages::root::unverified_root,
                pages::root::root_verify,
                pages::root::user_status,
                services::root::logout,
                services::root::validate,
                services::root::update_user_status,
                services::root::delete_user,
            ],
        )
        .mount(
            "/admin",
            routes![
                pages::admin::admin,
                pages::admin::admin_tag,
                pages::admin::admin_tags,
                pages::admin::admin_prods,
                pages::admin::admin_metrics,
                pages::admin::admin_orders,
                pages::admin::order_info,
                services::admin::refund_order,
                services::admin::finish_order,
                services::admin::verify_prod,
                services::admin::disable_prod,
                services::admin::add_tag,
                services::admin::remove_tag,
            ],
        )
        .mount(
            "/orders",
            routes![
                pages::orders::checkout,
                pages::orders::order_info_seller,
                pages::orders::order_info_alipay,
                pages::orders::order_info_paypal,
                services::orders::purchase,
                services::orders::progress_alipay,
                services::orders::cancel_order_alipay,
                services::orders::progress_paypal,
                services::orders::create_paypal_order,
                services::orders::capture_paypal_order,
                services::orders::cancel_order_paypal,
            ],
        )
        .mount(
            "/images",
            routes![
                services::images::upload,
                services::images::get,
                services::images::get_default
            ],
        )
        .mount("/i18n", routes![services::i18n::set_lang])
        .mount(
            "/digicons",
            routes![
                services::digicons::get_release_asset,
                services::digicons::get_git_repo,
                services::digicons::get_s3,
                pages::digicons::trace,
                pages::digicons::trace_unauthorized,
                services::digicons::delete_release_asset,
                services::digicons::delete_git_repo,
                services::digicons::delete_s3,
                services::digicons::upload_git_repo,
                services::digicons::upload_s3,
                services::digicons::update_digicon,
                services::digicons::create_digicon,
                pages::digicons::create_digicon_page,
                pages::digicons::all_digicons,
                pages::digicons::digicon_page,
                pages::digicons::digicons_center_not_permitted,
                services::digicons::add_digicon_mapping,
                services::digicons::remove_digicon_mapping,
            ],
        )
        .register(
            "/",
            catchers![
                pages::basics::page404,
                pages::basics::page422,
                pages::basics::page500
            ],
        )
}
