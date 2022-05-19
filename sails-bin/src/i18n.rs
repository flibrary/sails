pub use gettext::*;
use rocket::{
    http::{Cookie as HttpCookie, CookieJar, SameSite},
    response::Redirect,
};
use std::sync::Arc;

const ACCEPT_LANG: &str = "Accept-Language";

/// A request guard to get the right translation catalog for the current request.
pub struct I18n {
    /// The catalog containing the translated messages, in the correct locale for this request.
    pub catalog: Catalog,
    /// The language of the current request.
    pub lang: &'static str,
}

pub type Translations = Vec<(&'static str, Catalog)>;

use rocket::{
    http::Status,
    request::{FromRequest, Outcome, Request},
};

#[rocket::async_trait]
impl<'r> FromRequest<'r> for I18n {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let langs = req
            .rocket()
            .state::<Translations>()
            .expect("Couldn't retrieve translations because they are not managed by Rocket.");

        let lang: Arc<str> = if let Some(lang) = req
            .cookies()
            .get("lang")
            .map(|cookie| cookie.value().to_string())
        {
            // Make sure the language stored in cookie is supported
            if langs.iter().any(|l| l.0 == lang) {
                lang.into()
            } else {
                "en".into()
            }
        } else {
            req.headers()
                .get_one(ACCEPT_LANG)
                .unwrap_or("en".into())
                .split(',')
                .filter_map(|lang| {
                    lang
                        // Get the locale, not the country code
                        .split(|c| c == '-' || c == ';')
                        .next()
                })
                // Get the first requested locale we support
                .find(|lang| langs.iter().any(|l| l.0 == *lang))
                .unwrap_or("en".into())
                .into()
        };

        match langs.iter().find(|l| l.0 == &*lang) {
            Some(translation) => Outcome::Success(I18n {
                catalog: translation.1.clone(),
                lang: translation.0,
            }),
            None => Outcome::Failure((Status::InternalServerError, ())),
        }
    }
}

#[get("/set?<lang>")]
pub async fn set_lang(jar: &CookieJar<'_>, lang: String) -> Redirect {
    let cookie = HttpCookie::build("lang", lang)
        .secure(true)
        .same_site(SameSite::Strict)
        .finish();
    // Successfully validated, set cookie.
    jar.add(cookie);
    Redirect::to(uri!("/"))
}
