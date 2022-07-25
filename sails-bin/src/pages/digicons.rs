use crate::{
    infras::{aead::AeadKey, guards::*, i18n::I18n},
    DbConn, IntoFlash,
};
use askama::Template;
use chacha20poly1305::Nonce;
use chrono::NaiveDateTime;
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    digicons::*,
    error::SailsDbError,
    products::{ProductFinder, ProductInfo},
    users::{UserFinder, UserInfo},
};

#[derive(Template)]
#[template(path = "digicons/digicons.html")]
pub struct AllDigiconsPage {
    i18n: I18n,
    digicons: Vec<Digicon>,
}

#[get("/", rank = 2)]
pub async fn all_digicons(
    i18n: I18n,
    user: UserIdGuard<Cookie>,
    // TODO: granular permission
    _guard: Auth<CanCreateDigicon>,
    conn: DbConn,
) -> Result<AllDigiconsPage, Flash<Redirect>> {
    Ok(AllDigiconsPage {
        i18n,
        digicons: conn
            .run(move |c| Digicons::list_all_readable(c, &user.id))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[get("/", rank = 3)]
pub async fn digicons_center_not_permitted() -> Redirect {
    Redirect::to(uri!(crate::pages::basics::joinus))
}

#[derive(Template)]
#[template(path = "digicons/digicon.html")]
pub struct DigiconPage {
    i18n: I18n,
    digicon: Digicon,
    // bool represents wether it has already got mapping
    prods: Vec<(ProductInfo, bool)>,
}

#[get("/?<digicon_id>", rank = 1)]
pub async fn digicon_page(
    i18n: I18n,
    user: UserIdGuard<Cookie>,
    _guard_read: Auth<DigiconReadable>,
    _guard_write: Auth<DigiconWritable>,
    digicon_id: DigiconGuard,
    conn: DbConn,
) -> Result<DigiconPage, Flash<Redirect>> {
    let id = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    let digicon = id.clone();
    let prods = conn
        .run(move |c| -> Result<Vec<(ProductInfo, bool)>, SailsDbError> {
            Ok(ProductFinder::list_info(c)
                .map(|x| {
                    x.into_iter()
                        .filter(|p| p.writable(c, &user.id).unwrap_or(false))
                })?
                .map(|p| {
                    let is_mapped =
                        DigiconMappingFinder::has_mapping(c, &digicon, &p.to_id()).unwrap_or(false);
                    (p, is_mapped)
                })
                .collect())
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(DigiconPage {
        i18n,
        digicon: id,
        prods,
    })
}

#[derive(Template)]
#[template(path = "digicons/create_digicon.html")]
pub struct CreateDigiconPage {
    i18n: I18n,
}

#[get("/create_digicon")]
pub async fn create_digicon_page(i18n: I18n, _guard: Auth<CanCreateDigicon>) -> CreateDigiconPage {
    CreateDigiconPage { i18n }
}

#[derive(Template)]
#[template(path = "digicons/trace.html")]
pub struct TraceInfo {
    i18n: I18n,
    time: NaiveDateTime,
    digicon: Digicon,
    user: UserInfo,
}

#[get("/trace", rank = 2)]
pub async fn trace_unauthorized() -> Redirect {
    Redirect::to(uri!(
        "https://flibrary.info/store",
        crate::pages::store::home_page
    ))
}

#[get("/trace?<cipher>&<nonce>", rank = 1)]
pub async fn trace(
    i18n: I18n,
    _auth: Role<Admin>,
    aead: &State<AeadKey>,
    cipher: String,
    nonce: String,
    db: DbConn,
) -> Result<TraceInfo, Flash<Redirect>> {
    let cipher = base64::decode_config(&cipher, base64::URL_SAFE).into_flash(uri!("/"))?;
    let nonce = Nonce::clone_from_slice(
        &base64::decode_config(&nonce, base64::URL_SAFE).into_flash(uri!("/"))?,
    );

    let decrypted: Vec<String> =
        String::from_utf8(aead.decrypt(&cipher, &nonce).into_flash(uri!("/"))?)
            .into_flash(uri!("/"))?
            .split(':')
            .map(|x| x.to_string())
            .collect();

    let time = NaiveDateTime::from_timestamp(decrypted[0].parse().into_flash(uri!("/"))?, 0);

    let (digicon, user) = db
        .run(move |c| -> Result<_, SailsDbError> {
            Ok((
                Digicons::find_by_id(c, &decrypted[1])?,
                UserFinder::new(c, None).id(&decrypted[2]).first_info()?,
            ))
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(TraceInfo {
        i18n,
        time,
        digicon,
        user,
    })
}
