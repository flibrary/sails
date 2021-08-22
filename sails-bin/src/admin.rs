use crate::{
    guards::{AdminGuard, MutableBookInfoGuard, OrderIdGuard, OrderInfoGuard},
    DbConn, IntoFlash, Msg,
};
use askama::Template;
use rocket::{
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{
    enums::{ProductStatus, TransactionStatus},
    error::SailsDbError,
    products::{ProductFinder, ProductInfo},
    transactions::*,
    Cmp,
};

#[derive(Template)]
#[template(path = "admin/tx.html")]
pub struct AdminTxPage {
    inner: Msg,
    paid_tx: Vec<(ProductInfo, TransactionInfo)>,
    placed_tx: Vec<(ProductInfo, TransactionInfo)>,
    refunded_tx: Vec<(ProductInfo, TransactionInfo)>,
    finished_tx: Vec<(ProductInfo, TransactionInfo)>,
}

// If the user has already been verified, show him the root dashboard
#[get("/tx")]
pub async fn admin_tx(
    flash: Option<FlashMessage<'_>>,
    _guard: AdminGuard,
    conn: DbConn,
) -> Result<AdminTxPage, Flash<Redirect>> {
    let paid_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Paid, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let refunded_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Refunded, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let placed_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Placed, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let finished_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Finished, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    Ok(AdminTxPage {
        inner: Msg::from_flash(flash),
        paid_tx,
        placed_tx,
        refunded_tx,
        finished_tx,
    })
}

#[derive(Template)]
#[template(path = "admin/admin.html")]
pub struct AdminPage {
    inner: Msg,
    normal_books: Vec<ProductInfo>,
    verified_books: Vec<ProductInfo>,
    disabled_books: Vec<ProductInfo>,
}

// If the user has already been verified, show him the root dashboard
#[get("/books")]
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
    info: MutableBookInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.info
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
    info: MutableBookInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.info
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
    info: MutableBookInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.info
            .set_product_status(ProductStatus::Normal)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin))?;
    Ok(Redirect::to(uri!("/admin", admin)))
}

#[get("/refund_order")]
pub async fn refund_order(
    _guard: AdminGuard,
    id: OrderIdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| id.id.refund(c))
        .await
        .into_flash(uri!("/admin", admin_tx))?;
    Ok(Redirect::to(uri!("/admin", admin_tx)))
}

#[get("/finish_order")]
pub async fn finish_order(
    _guard: AdminGuard,
    info: OrderInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.order_info
            .set_transaction_status(TransactionStatus::Finished)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin_tx))?;
    Ok(Redirect::to(uri!("/admin", admin_tx)))
}

#[get("/confirm_order")]
pub async fn confirm_order(
    _guard: AdminGuard,
    info: OrderInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.order_info
            .set_transaction_status(TransactionStatus::Paid)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin_tx))?;
    Ok(Redirect::to(uri!("/admin", admin_tx)))
}
