use crate::{guards::*, sanitize_html, DbConn, IntoFlash};
use askama::Template;
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};
use rocket_i18n::I18n;
use sails_db::{digicons::*, error::SailsDbError, products::*, transactions::*, users::*};

type OrderEntry = (ProductInfo, TransactionInfo);

#[derive(Debug, FromForm, Clone)]
pub struct PartialUserFormOwned {
    pub name: String,
    pub school: String,
    pub description: Option<String>,
}

#[post("/update_user", data = "<info>")]
pub async fn update_user(
    user: UserInfoGuard<Cookie>,
    info: Form<PartialUserFormOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let mut info = info.into_inner();
    info.description = info.description.map(|d| sanitize_html(&d));
    conn.run(move |c| {
        user.info
            .set_description(info.description)
            .set_name(info.name)
            .set_school(info.school)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/user", portal)))
}

#[derive(Template)]
#[template(path = "user/update_user_page.html")]
pub struct UpdateUserPage {
    i18n: I18n,
    user: UserInfo,
}

#[get("/update_user_page")]
pub async fn update_user_page(i18n: I18n, user: UserInfoGuard<Cookie>) -> UpdateUserPage {
    UpdateUserPage {
        i18n,
        user: user.info,
    }
}

#[derive(Template)]
#[template(path = "user/portal_guest.html")]
pub struct PortalGuestPage {
    i18n: I18n,
    user: UserInfo,
    prods_owned: Vec<ProductInfo>,
}

#[derive(Template)]
#[template(path = "user/portal.html")]
pub struct PortalPage {
    i18n: I18n,
    user: UserInfo,
    digicons_owned: Vec<Digicon>,
    prods_owned: Vec<ProductInfo>,
    orders_placed: Vec<OrderEntry>,
    orders_received: Vec<OrderEntry>,
}

#[get("/?<user_id>", rank = 1)]
pub async fn portal_guest(
    i18n: I18n,
    _signedin: UserIdGuard<Cookie>,
    user_id: UserGuard,
    conn: DbConn,
) -> Result<PortalGuestPage, Flash<Redirect>> {
    let user = user_id.to_info_param(&conn).await.into_flash(uri!("/"))?;

    let uid = user.info.to_id();
    let prods_owned = conn
        .run(move |c| -> Result<Vec<ProductInfo>, SailsDbError> {
            let prods_owned = ProductFinder::new(c, None).seller(&uid).search_info()?;
            Ok(prods_owned)
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(PortalGuestPage {
        i18n,
        user: user.info,
        prods_owned,
    })
}

// The flash message is required here because we may get error from update_user
#[get("/", rank = 2)]
pub async fn portal(
    i18n: I18n,
    user: UserInfoGuard<Cookie>,
    conn: DbConn,
) -> Result<PortalPage, Flash<Redirect>> {
    let uid = user.info.to_id();
    #[allow(clippy::type_complexity)]
    let (digicons_owned, prods_owned, orders_placed, orders_received) = conn
        .run(move |c| -> Result<_, SailsDbError> {
            let prods_owned = ProductFinder::new(c, None).seller(&uid).search_info()?;
            let digicons_owned = Digicons::list_all_authorized(c, &uid)?;

            let orders_placed = TransactionFinder::new(c, None)
                .buyer(&uid)
                .search_info()?
                .into_iter()
                .map(|x| {
                    let product = ProductFinder::new(c, None)
                        .id(x.get_product())
                        .first_info()?;
                    Ok((product, x))
                })
                .collect::<Result<Vec<OrderEntry>, SailsDbError>>()?;

            let orders_received = TransactionFinder::new(c, None)
                .seller(&uid)
                .search_info()?
                .into_iter()
                .map(|x| {
                    let product = ProductFinder::new(c, None)
                        .id(x.get_product())
                        .first_info()?;
                    Ok((product, x))
                })
                .collect::<Result<Vec<OrderEntry>, SailsDbError>>()?;
            Ok((digicons_owned, prods_owned, orders_placed, orders_received))
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(PortalPage {
        i18n,
        user: user.info,
        digicons_owned,
        orders_placed,
        orders_received,
        prods_owned,
    })
}

#[get("/", rank = 3)]
pub async fn portal_unsigned() -> Redirect {
    Redirect::to(uri!("/user", super::auth::signin))
}
