use crate::{guards::*, sanitize_html, DbConn, IntoFlash};
use askama::Template;
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};
use sails_db::{
    categories::{Categories, Category, CtgTrait},
    error::SailsDbError,
    products::*,
    transactions::*,
    users::*,
};

type ProductEntry = (ProductInfo, Category);
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
    user: UserInfo,
}

#[get("/update_user_page")]
pub async fn update_user_page(user: UserInfoGuard<Cookie>) -> UpdateUserPage {
    UpdateUserPage { user: user.info }
}

#[derive(Template)]
#[template(path = "user/portal_guest.html")]
pub struct PortalGuestPage {
    user: UserInfo,
    books_operated: Vec<ProductEntry>,
    books_owned: Vec<ProductEntry>,
}

#[derive(Template)]
#[template(path = "user/portal.html")]
pub struct PortalPage {
    user: UserInfo,
    books_operated: Vec<ProductEntry>,
    books_owned: Vec<ProductEntry>,
    orders_placed: Vec<OrderEntry>,
    orders_received: Vec<OrderEntry>,
}

#[get("/?<user_id>", rank = 1)]
pub async fn portal_guest(
    _signedin: UserIdGuard<Cookie>,
    user_id: UserGuard,
    conn: DbConn,
) -> Result<PortalGuestPage, Flash<Redirect>> {
    let user = user_id.to_info_param(&conn).await.into_flash(uri!("/"))?;

    let uid = user.info.get_id().to_string();
    let (books_operated, books_owned) = conn
        .run(
            move |c| -> Result<(Vec<ProductEntry>, Vec<ProductEntry>), SailsDbError> {
                let books_operated = ProductFinder::new(c, None)
                    .seller(&uid)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        let ctg = Categories::find_by_id(c, x.get_category_id())?;
                        Ok((x, ctg))
                    })
                    .chain(
                        ProductFinder::new(c, None)
                            .delegator(&uid)
                            .search_info()?
                            .into_iter()
                            .map(|x| {
                                let ctg = Categories::find_by_id(c, x.get_category_id())?;
                                Ok((x, ctg))
                            }),
                    )
                    .collect::<Result<Vec<ProductEntry>, SailsDbError>>()?;
                let books_owned = ProductFinder::new(c, None)
                    .owner(&uid)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        let ctg = Categories::find_by_id(c, x.get_category_id())?;
                        Ok((x, ctg))
                    })
                    .collect::<Result<Vec<ProductEntry>, SailsDbError>>()?;
                Ok((books_operated, books_owned))
            },
        )
        .await
        .into_flash(uri!("/"))?;

    Ok(PortalGuestPage {
        user: user.info,
        books_operated,
        books_owned,
    })
}

// The flash message is required here because we may get error from update_user
#[get("/", rank = 2)]
pub async fn portal(
    user: UserInfoGuard<Cookie>,
    conn: DbConn,
) -> Result<PortalPage, Flash<Redirect>> {
    let uid = user.info.get_id().to_string();
    #[allow(clippy::type_complexity)]
    let (books_operated, books_owned, orders_placed, orders_received) = conn
        .run(
            move |c| -> Result<
                (
                    Vec<ProductEntry>,
                    Vec<ProductEntry>,
                    Vec<OrderEntry>,
                    Vec<OrderEntry>,
                ),
                SailsDbError,
            > {
                let books_operated = ProductFinder::new(c, None)
                    .seller(&uid)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        let ctg = Categories::find_by_id(c, x.get_category_id())?;
                        Ok((x, ctg))
                    })
                    .chain(
                        ProductFinder::new(c, None)
                            .delegator(&uid)
                            .search_info()?
                            .into_iter()
                            .map(|x| {
                                let ctg = Categories::find_by_id(c, x.get_category_id())?;
                                Ok((x, ctg))
                            }),
                    )
                    .collect::<Result<Vec<ProductEntry>, SailsDbError>>()?;
                let books_owned = ProductFinder::new(c, None)
                    .owner(&uid)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        let ctg = Categories::find_by_id(c, x.get_category_id())?;
                        Ok((x, ctg))
                    })
                    .collect::<Result<Vec<ProductEntry>, SailsDbError>>()?;

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
                Ok((books_operated, books_owned, orders_placed, orders_received))
            },
        )
        .await
        .into_flash(uri!("/"))?;

    Ok(PortalPage {
        user: user.info,
        orders_placed,
        orders_received,
        books_operated,
        books_owned,
    })
}

#[get("/", rank = 3)]
pub async fn portal_unsigned() -> Redirect {
    Redirect::to(uri!("/user", super::auth::signin))
}
