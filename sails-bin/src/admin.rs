use crate::{
    guards::{AdminGuard, NonSoldBookInfoGuard},
    DbConn, IntoFlash, Msg,
};
use askama::Template;
use rocket::{
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{
    enums::ProductStatus,
    products::{ProductFinder, ProductInfo},
    Cmp,
};

#[derive(Template)]
#[template(path = "admin/admin.html")]
pub struct AdminPage {
    inner: Msg,
    normal_books: Vec<ProductInfo>,
    verified_books: Vec<ProductInfo>,
    disabled_books: Vec<ProductInfo>,
}

// If the user has already been verified, show him the root dashboard
#[get("/", rank = 1)]
pub async fn admin(
    flash: Option<FlashMessage<'_>>,
    _guard: AdminGuard,
    conn: DbConn,
) -> Result<AdminPage, Flash<Redirect>> {
    let normal_books = conn
        .run(|c| {
            ProductFinder::new(c, None)
                .status(ProductStatus::Normal, Cmp::Equal)
                .search_info()
        })
        .await
        .into_flash(uri!("/"))?;

    let disabled_books = conn
        .run(|c| {
            ProductFinder::new(c, None)
                .status(ProductStatus::Disabled, Cmp::Equal)
                .search_info()
        })
        .await
        .into_flash(uri!("/"))?;

    let verified_books = conn
        .run(|c| {
            ProductFinder::new(c, None)
                .status(ProductStatus::Verified, Cmp::Equal)
                .search_info()
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(AdminPage {
        normal_books,
        disabled_books,
        verified_books,
        inner: Msg::from_flash(flash),
    })
}

#[get("/verify_book")]
pub async fn verify_book(
    _guard: AdminGuard,
    info: NonSoldBookInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.inner
            .book_info
            .set_product_status(ProductStatus::Verified)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin))?;
    Ok(Redirect::to(uri!("/admin", admin)))
}

#[get("/disable_book")]
pub async fn disable_book(
    _guard: AdminGuard,
    info: NonSoldBookInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.inner
            .book_info
            .set_product_status(ProductStatus::Disabled)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin))?;
    Ok(Redirect::to(uri!("/admin", admin)))
}

#[get("/normalize_book")]
pub async fn normalize_book(
    _guard: AdminGuard,
    info: NonSoldBookInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.inner
            .book_info
            .set_product_status(ProductStatus::Normal)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin))?;
    Ok(Redirect::to(uri!("/admin", admin)))
}
