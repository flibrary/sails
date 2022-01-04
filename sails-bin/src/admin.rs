use crate::{
    guards::{Admin, BookInfoGuard, OrderIdGuard, OrderInfoGuard, Role},
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
    products::{MutableProductInfo, ProductFinder, ProductInfo},
    transactions::*,
    users::{UserFinder, UserStats},
    Cmp,
};

#[derive(Template)]
#[template(path = "admin/metrics.html")]
pub struct AdminMetricsPage {
    pub order: TxStats,
    pub user: UserStats,
}

// To prevent deadlock, redirect all errors back to index as this is the default route for `/admin`
#[get("/metrics")]
pub async fn admin_metrics(
    _guard: Role<Admin>,
    conn: DbConn,
) -> Result<AdminMetricsPage, Flash<Redirect>> {
    Ok(AdminMetricsPage {
        order: conn
            .run(|c| TransactionFinder::stats(c, None))
            .await
            .into_flash(uri!("/"))?,
        user: conn
            .run(|c| UserFinder::stats(c))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[derive(Template)]
#[template(path = "admin/orders.html")]
pub struct AdminOrdersPage {
    inner: Msg,
    paid_tx: Vec<(ProductInfo, TransactionInfo)>,
    placed_tx: Vec<(ProductInfo, TransactionInfo)>,
    refunded_tx: Vec<(ProductInfo, TransactionInfo)>,
    finished_tx: Vec<(ProductInfo, TransactionInfo)>,
}

// If the user has already been verified, show him the root dashboard
#[get("/orders")]
pub async fn admin_orders(
    flash: Option<FlashMessage<'_>>,
    _guard: Role<Admin>,
    conn: DbConn,
) -> Result<AdminOrdersPage, Flash<Redirect>> {
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

    Ok(AdminOrdersPage {
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
pub async fn admin_books(
    flash: Option<FlashMessage<'_>>,
    _guard: Role<Admin>,
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
    _guard: Role<Admin>,
    info: BookInfoGuard<MutableProductInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.book_info
            .set_product_status(ProductStatus::Verified)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin_books))?;
    Ok(Redirect::to(uri!("/admin", admin_books)))
}

#[get("/disable_book")]
pub async fn disable_book(
    _guard: Role<Admin>,
    info: BookInfoGuard<MutableProductInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.book_info
            .set_product_status(ProductStatus::Disabled)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin_books))?;
    Ok(Redirect::to(uri!("/admin", admin_books)))
}

#[get("/normalize_book")]
pub async fn normalize_book(
    _guard: Role<Admin>,
    info: BookInfoGuard<MutableProductInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.book_info
            .set_product_status(ProductStatus::Normal)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin_books))?;
    Ok(Redirect::to(uri!("/admin", admin_books)))
}

#[get("/refund_order")]
pub async fn refund_order(
    _guard: Role<Admin>,
    id: OrderIdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| id.id.refund(c))
        .await
        .into_flash(uri!("/admin", admin_orders))?;
    Ok(Redirect::to(uri!("/admin", admin_orders)))
}

#[get("/finish_order")]
pub async fn finish_order(
    _guard: Role<Admin>,
    info: OrderInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.order_info
            .set_transaction_status(TransactionStatus::Finished)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin_orders))?;
    Ok(Redirect::to(uri!("/admin", admin_orders)))
}

#[get("/confirm_order")]
pub async fn confirm_order(
    _guard: Role<Admin>,
    info: OrderInfoGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(|c| {
        info.order_info
            .set_transaction_status(TransactionStatus::Paid)
            .update(c)
    })
    .await
    .into_flash(uri!("/admin", admin_orders))?;
    Ok(Redirect::to(uri!("/admin", admin_orders)))
}

#[get("/")]
pub async fn admin(_guard: Role<Admin>) -> Redirect {
    Redirect::to(uri!("/admin", admin_metrics))
}
