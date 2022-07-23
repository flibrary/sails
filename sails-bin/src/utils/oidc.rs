use anyhow::anyhow;
use openidconnect::{
    core::{
        CoreAuthenticationFlow, CoreClient, CoreIdToken, CoreIdTokenClaims, CoreProviderMetadata,
    },
    reqwest::async_http_client,
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, RedirectUrl, Scope,
    TokenResponse,
};
use rocket::{
    fairing::{AdHoc, Fairing},
    figment::Figment,
    form::{Form, FromForm},
    http::{Cookie as HttpCookie, CookieJar, SameSite, Status},
    outcome::{IntoOutcome, Outcome},
    request::{self, FromRequest, Request},
    response::Redirect,
};

pub const STATE_COOKIE_NAME: &str = "sails_oidc_state";
pub const NONCE_COOKIE_NAME: &str = "sails_oidc_nonce";
pub const ID_TOKEN_COOKIE_NAME: &str = "sails_oidc_id_token";

pub struct OIDCClient {
    client: CoreClient,
    pub logout_redirect_uri: String,
}

impl OIDCClient {
    async fn from_figment(figment: &Figment) -> Result<Self, anyhow::Error> {
        #[derive(serde::Deserialize)]
        struct Config {
            discovery_uri: String,
            redirect_uri: String,
            logout_redirect_uri: String,
            client_id: String,
            client_secret: String,
        }

        let config: Config = figment.extract_inner("oidc")?;

        let provider_metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(config.discovery_uri)?,
            async_http_client,
        )
        .await?;

        // Create an OpenID Connect client by specifying the client ID, client secret, authorization URL
        // and token URL.
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(config.client_id),
            Some(ClientSecret::new(config.client_secret)),
        )
        // Set the URL the user will be redirected to after the authorization process.
        .set_redirect_uri(RedirectUrl::new(config.redirect_uri)?);

        Ok(Self {
            client,
            logout_redirect_uri: config.logout_redirect_uri,
        })
    }

    pub fn fairing() -> impl Fairing {
        AdHoc::try_on_ignite("oidc", move |rocket| async move {
            let client = match Self::from_figment(rocket.figment()).await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Failed on constructing OpenID Connect Client: {:?}", e);
                    return Err(rocket);
                }
            };
            Ok(rocket.manage(client))
        })
    }

    pub fn get_redirect(&self, cookies: &CookieJar<'_>, scopes: &[&str]) -> Redirect {
        // Generate the full authorization URL.
        let mut client = self.client.authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        );
        // Set the desired scopes.
        for scope in scopes {
            client = client.add_scope(Scope::new(scope.to_string()));
        }
        let (auth_url, csrf_token, nonce) = client.url();

        // Private cookie is only accessible by us, so we are safe.
        cookies.add_private(
            HttpCookie::build(STATE_COOKIE_NAME, csrf_token.secret().to_string())
                .same_site(SameSite::Lax)
                .finish(),
        );

        cookies.add_private(
            HttpCookie::build(NONCE_COOKIE_NAME, nonce.secret().clone())
                .same_site(SameSite::Lax)
                .finish(),
        );

        Redirect::to(auth_url.as_str().to_string())
    }
}

pub struct OIDCTokenResponse {
    pub id_token: CoreIdToken,
    pub claims: CoreIdTokenClaims,
}

impl OIDCTokenResponse {
    async fn from_request<'r>(request: &'r Request<'_>) -> Result<Self, anyhow::Error> {
        let client = &request
            .rocket()
            .state::<OIDCClient>()
            .ok_or_else(|| anyhow!("Missing OIDC client"))?
            .client;

        // Parse the query data.
        let query = match request.uri().query() {
            Some(q) => q,
            None => {
                return Err(anyhow!("Missing query string in request"));
            }
        };

        #[derive(FromForm)]
        struct CallbackQuery {
            code: String,
            state: String,
        }

        let params = Form::<CallbackQuery>::parse_encoded(&query)
            .map_err(|e| anyhow!("Failed to parse OpenID Connect callback parameters: {}", e))?;

        // Verify that the given state is the same one in the cookie.
        // Begin a new scope so that cookies is not kept around too long.
        let cookies = request
            .guard::<&CookieJar<'_>>()
            .await
            .map_failure(|_| anyhow!("Missing cookie jar"))
            .ok_map_forward(|_| Err(anyhow!("Missing cookie jar")))?;

        match cookies.get_private(STATE_COOKIE_NAME) {
            Some(ref cookie) if cookie.value() == params.state => {
                cookies.remove_private(cookie.clone());
            }
            Some(cookie) => {
                warn!("The OAuth2 state returned from the server did not match the stored state.");
                cookies.remove_private(cookie);
                return Err(anyhow!(
                    "The OAuth2 state returned from the server did match the stored state."
                ));
            }
            None => {
                error!(
                    "The OAuth2 state cookie was missing. It may have been blocked by the client?"
                );

                return Err(anyhow!(
                    "The OAuth2 state returned from the server did match the stored state."
                ));
            }
        }

        // Get Nonce from cookie
        let nonce = match cookies.get_private(NONCE_COOKIE_NAME) {
            Some(cookie) => {
                let n = Nonce::new(cookie.value().to_string());
                cookies.remove_private(cookie);
                n
            }
            None => {
                return Err(anyhow!(
                    "The OAuth2 state cookie was missing. It may have been blocked by the client?"
                ))
            }
        };

        let token_response = client
            .exchange_code(AuthorizationCode::new(params.code))
            .request_async(async_http_client)
            .await?;

        // Extract the ID token claims after verifying its authenticity and nonce.
        let id_token = token_response
            .id_token()
            .ok_or_else(|| anyhow!("Server did not return an ID token"))?;
        let claims = id_token.claims(&client.id_token_verifier(), &nonce)?;

        // Add back id_token for future retrieval
        cookies.add_private(
            HttpCookie::build(ID_TOKEN_COOKIE_NAME, id_token.to_string())
                .secure(true)
                .same_site(SameSite::Strict)
                .finish(),
        );

        Ok(Self {
            id_token: id_token.clone(),
            claims: claims.clone(),
        })
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OIDCTokenResponse {
    type Error = anyhow::Error;

    // TODO: Decide if BadRequest is the appropriate error code.
    // TODO: What do providers do if they *reject* the authorization?
    /// Handle the redirect callback, delegating to the Adapter to perform the
    /// token exchange.
    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        match Self::from_request(request).await {
            Err(e) => Outcome::Failure((Status::BadRequest, e)),
            Ok(s) => Outcome::Success(s),
        }
    }
}

pub struct OIDCIdToken {
    pub id_token: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OIDCIdToken {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        request
            .cookies()
            .get_private(ID_TOKEN_COOKIE_NAME)
            .map(|cookie| OIDCIdToken {
                id_token: cookie.value().to_string(),
            })
            .or_forward(())
    }
}
