use ammonia::Builder;
use once_cell::sync::Lazy;
use rocket::{
    fairing::{AdHoc, Fairing},
    http::{uri::Reference, Header, Status},
    request::FlashMessage,
    response::{self, Flash, Redirect},
};
use rust_embed::RustEmbed;
use serde::Deserialize;
use std::{convert::TryInto, ffi::OsStr, io::Cursor, path::PathBuf};

// A short hand message <-> flash conversion
pub struct Msg {
    pub flash: Option<String>,
}

impl Msg {
    // Construct a message from a flash message
    pub fn from_flash(flash: Option<FlashMessage<'_>>) -> Self {
        Self {
            flash: flash.map(|f| format!("{}: {}", f.kind(), f.message())),
        }
    }
}

pub fn sanitize_html(html: &str) -> String {
    SANITIZER.clean(html).to_string()
}

// Comrak options. We selectively enabled a few GFM standards.
pub static SANITIZER: Lazy<Builder> = Lazy::new(|| {
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

#[derive(RustEmbed)]
#[folder = "static/"]
pub struct Asset;

pub struct StaticFile(pub PathBuf);

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

pub fn create_fairing<'a, T: Deserialize<'a> + Sync + Send + 'static>(
    name: &'static str,
) -> impl Fairing {
    AdHoc::try_on_ignite(name, move |rocket| async move {
        let config: T = match rocket.figment().extract_inner(name) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Invalid configuration: {:?}", e);
                return Err(rocket);
            }
        };

        Ok(rocket.manage(config))
    })
}
