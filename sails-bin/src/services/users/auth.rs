use crate::{
    infras::{
        i18n::I18n,
        oidc::{OIDCClient, OIDCIdToken, OIDCTokenResponse, ID_TOKEN_COOKIE_NAME},
    },
    pages::users::*,
    DbConn, IntoFlash,
};
use rocket::{
    http::{Cookie as HttpCookie, CookieJar, SameSite},
    response::{Flash, Redirect},
    State,
};
use sails_db::{error::SailsDbError, users::*};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    email: String,
    name: String,
}

// This would be mounted under namespace `user` and eventually become `/user/signin`
#[get("/signin")]
pub async fn signin(client: &State<OIDCClient>, cookies: &CookieJar<'_>) -> Redirect {
    client.get_redirect(cookies, &["email", "profile"])
}

#[get("/signin_callback")]
pub async fn signin_callback(
    i18n: I18n,
    token: OIDCTokenResponse,
    jar: &CookieJar<'_>,
    conn: DbConn,
) -> Result<SignInConfirmation, Flash<Redirect>> {
    let name = token
        .claims
        .name()
        .ok_or("No name provided by FLibrary ID")
        .into_flash(uri!("/"))?
        .get(None)
        .ok_or("Name without locale specification is not provided by FLibrary ID")
        .into_flash(uri!("/"))?
        .as_str()
        .to_string();
    let email = token
        .claims
        .email()
        .ok_or("No name provided by FLibrary ID")
        .into_flash(uri!("/"))?
        .as_str()
        .to_string();

    let name_cloned = name.clone();
    let email_cloned = email.clone();

    // Create user if user not found in our local database
    conn.run(move |c| -> Result<(), SailsDbError> {
        if matches!(UserId::find(c, &email), Err(SailsDbError::QueryError(_))) {
            UserForm::new(&email, &name, "", None).to_ref()?.create(c)?;
        }
        Ok(())
    })
    .await
    .into_flash(uri!("/"))?;

    // Set the private session cookie
    let cookie = HttpCookie::build("uid", email_cloned)
        .secure(true)
        .same_site(SameSite::Strict)
        .finish();
    // Successfully validated, set private cookie.
    jar.add_private(cookie);

    Ok(SignInConfirmation {
        i18n,
        name: name_cloned,
    })
}

#[get("/logout", rank = 1)]
pub async fn logout(
    jar: &CookieJar<'_>,
    id_token: OIDCIdToken,
    client: &State<OIDCClient>,
) -> Redirect {
    if let Some(uid) = jar.get_private("uid") {
        jar.remove_private(uid);
    } else {
        // No UID specified, do nothing
    }

    if let Some(uid) = jar.get_private(ID_TOKEN_COOKIE_NAME) {
        jar.remove_private(uid);
    } else {
        // No UID specified, do nothing
    }

    // Redirect back to home
    Redirect::to(format!(
        "{}&id_token_hint={}",
        client.logout_redirect_uri, id_token.id_token
    ))
}

#[get("/logout", rank = 2)]
pub async fn logout_fallback(jar: &CookieJar<'_>) -> Redirect {
    if let Some(uid) = jar.get_private("uid") {
        jar.remove_private(uid);
    } else {
        // No UID specified, do nothing
    }

    if let Some(id_token) = jar.get_private(ID_TOKEN_COOKIE_NAME) {
        jar.remove_private(id_token);
    } else {
        // No id_token cookie, do noting
    }

    // Redirect back to home
    Redirect::to(uri!("/"))
}
