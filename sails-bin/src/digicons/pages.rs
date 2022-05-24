use crate::{guards::*, i18n::I18n, DbConn, IntoFlash};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{
    digicons::*,
    error::SailsDbError,
    products::{ProductFinder, ProductInfo},
};

#[get("/remove_digicon?<digicon_id>&<prod_id>")]
pub async fn remove_digicon(
    _digicon_guard: Auth<DigiconWritable>,
    _prod_guard: Auth<ProdWritable>,
    digicon_id: DigiconGuard,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    let digicon_cloned = digicon.clone();
    conn.run(move |c| {
        DigiconMappingFinder::new(c, None)
            .product(&prod.prod_id)
            .digicon(&digicon)
            .first()
            .map(|x| x.delete(c))
    })
    .await
    .into_flash(uri!("/"))?
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!(
        "/digicons",
        digicon_page(digicon_cloned.get_id())
    )))
}

#[get("/add_digicon?<digicon_id>&<prod_id>")]
pub async fn add_digicon(
    _digicon_guard: Auth<DigiconWritable>,
    _prod_guard: Auth<ProdWritable>,
    digicon_id: DigiconGuard,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    let digicon_cloned = digicon.clone();
    conn.run(move |c| DigiconMapping::create(c, &digicon, &prod.prod_id))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!(
        "/digicons",
        digicon_page(digicon_cloned.get_id())
    )))
}

#[derive(Template)]
#[template(path = "digicons/digicons.html")]
pub struct AllDigiconsPage {
    i18n: I18n,
    digicons: Vec<Digicon>,
}

#[get("/", rank = 2)]
pub async fn all_digicons(
    i18n: I18n,
    // TODO: granular permission
    _guard: Auth<CanCreateDigicon>,
    conn: DbConn,
) -> Result<AllDigiconsPage, Flash<Redirect>> {
    Ok(AllDigiconsPage {
        i18n,
        digicons: conn
            .run(|c| Digicons::list_all(c))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[get("/", rank = 3)]
pub async fn digicons_center_not_permitted() -> Redirect {
    Redirect::to(uri!(crate::joinus))
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
    _guard: Auth<DigiconWritable>,
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
