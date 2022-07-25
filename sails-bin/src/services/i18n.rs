use rocket::{
    http::{Cookie, CookieJar, SameSite},
    response::Redirect,
};

#[get("/set?<lang>")]
pub async fn set_lang(jar: &CookieJar<'_>, lang: String) -> Redirect {
    let cookie = Cookie::build("lang", lang)
        .secure(true)
        // When redirected back from FLibrary ID, unless the samesite restriction is None, firefox doesn't send lang cookie, resulting in mismatched user experience.
        .same_site(SameSite::None)
        .finish();
    // Successfully validated, set cookie.
    jar.add(cookie);
    Redirect::to(uri!("/"))
}
