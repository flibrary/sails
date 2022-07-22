use crate::{utils::i18n::I18n, DbConn, FLibraryID, IntoFlash};
use askama::Template;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use rocket::{
    http::{Cookie as HttpCookie, CookieJar, SameSite},
    response::{Flash, Redirect},
};
use rocket_oauth2::{OAuth2, TokenResponse};
use sails_db::{error::SailsDbError, users::*};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Claims {
    email: String,
    name: String,
}

// This would be mounted under namespace `user` and eventually become `/user/signin`
#[get("/signin")]
pub async fn signin(
    oauth2: OAuth2<FLibraryID>,
    cookies: &CookieJar<'_>,
) -> Result<Redirect, Flash<Redirect>> {
    oauth2
        .get_redirect(cookies, &["email", "profile"])
        .into_flash(uri!("/"))
}

#[derive(Template)]
#[template(path = "user/signin_confirmation.html")]
pub struct SignInConfirmation {
    i18n: I18n,
    name: String,
}

#[get("/signin_callback")]
pub async fn signin_callback(
    i18n: I18n,
    token: TokenResponse<FLibraryID>,
    jar: &CookieJar<'_>,
    conn: DbConn,
) -> Result<SignInConfirmation, Flash<Redirect>> {
    // Key from JWKs.json from keycloak: https://id.flibrary.info/realms/Customers/protocol/openid-connect/certs
    let key = DecodingKey::from_rsa_components("78WjV0F2wpZnHGFYP7h1LizDSaQAVthaW4_ASi8ya6lQtruT8HSzwa7hUDlXoiRKiLR2mvz73WuglHXUpQYQ8LXVK7sAEY9FH98SDAqk5tLCT9vths6eM12DZFQnDJD0yhW7L6F5BGQPydjGcfyHfqwY5cjFzO097x7kuyUND6-Jt8a5jS9rkVEEFvaIU5nfv5OTLQMRLtRMu6O_VLLBkDZH7wnbWoQ5wKDpYcEKyMSFxlAZEMYRNHAF2-xoP3QCVuVf4vwiGWSWCExos2jwm8CCsAX_E5iyorC2r1DE6sv1FS5QVLzbWe93TdJw0Rx3i_hh_fb_HCFv1yYmX60EfQ", "AQAB").into_flash(uri!("/"))?;

    let claims = decode::<Claims>(
        token.access_token(),
        &key,
        &Validation::new(Algorithm::RS256),
    )
    .into_flash(uri!("/"))?
    .claims;

    let claims_cloned = claims.clone();

    // Create user if user not found in our local database
    conn.run(move |c| -> Result<(), SailsDbError> {
        if matches!(
            UserId::find(c, &claims.email),
            Err(SailsDbError::QueryError(_))
        ) {
            UserForm::new(&claims.email, &claims.name, "", None)
                .to_ref()?
                .create(c)?;
        }
        Ok(())
    })
    .await
    .into_flash(uri!("/"))?;

    // Set the private session cookie
    let cookie = HttpCookie::build("uid", claims_cloned.email)
        .secure(true)
        .same_site(SameSite::Strict)
        .finish();
    // Successfully validated, set private cookie.
    jar.add_private(cookie);

    Ok(SignInConfirmation {
        i18n,
        name: claims_cloned.name,
    })
}

#[get("/logout")]
pub async fn logout(jar: &CookieJar<'_>) -> Redirect {
    if let Some(uid) = jar.get_private("uid") {
        jar.remove_private(uid);
    } else {
        // No UID specified, do nothing
    }
    // Redirect back to home
    Redirect::to(uri!("/"))
}
