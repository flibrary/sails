use crate::infras::{
    basics::{Msg, StaticFile},
    i18n::I18n,
};
use askama::Template;
use rocket::{request::FlashMessage, response::Redirect};
use std::path::PathBuf;

#[derive(Template)]
#[template(path = "index.html")]
pub struct Index {
    i18n: I18n,
    inner: Msg,
}

#[get("/")]
pub async fn index<'a>(i18n: I18n, flash: Option<FlashMessage<'_>>) -> Index {
    Index {
        i18n,
        inner: Msg::from_flash(flash),
    }
}

#[derive(Template)]
#[template(path = "joinus.html")]
pub struct JoinUs {
    i18n: I18n,
}

#[get("/joinus")]
pub async fn joinus(i18n: I18n) -> JoinUs {
    JoinUs { i18n }
}

#[get("/<path..>")]
pub async fn get_file(path: PathBuf) -> StaticFile {
    StaticFile(path)
}

#[get("/favicon.ico")]
pub async fn get_icon() -> Redirect {
    Redirect::to(uri!("/static/favicon.ico"))
}

#[catch(404)]
pub async fn page404<'a>() -> Redirect {
    Redirect::to("/static/404.html")
}

#[catch(422)]
pub async fn page422<'a>() -> Redirect {
    Redirect::to("/static/422.html")
}

#[catch(500)]
pub async fn page500<'a>() -> Redirect {
    Redirect::to("/static/500.html")
}
