use crate::{
    alipay::{AlipayAppPrivKey, AlipayClient, CancelTrade, CancelTradeResp},
    guards::*,
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    enums::{ProductStatus, TransactionStatus},
    error::SailsDbError,
    products::{ProductFinder, ProductInfo},
    tags::*,
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
    paid_tx: Vec<(ProductInfo, TransactionInfo)>,
    placed_tx: Vec<(ProductInfo, TransactionInfo)>,
    refunded_tx: Vec<(ProductInfo, TransactionInfo)>,
    finished_tx: Vec<(ProductInfo, TransactionInfo)>,
}

// CustomerService or above can READ all orders
#[get("/orders")]
pub async fn admin_orders(
    _guard: Role<CustomerService>,
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
        paid_tx,
        placed_tx,
        refunded_tx,
        finished_tx,
    })
}

#[get("/remove_tag?<tag_id>&<book_id>")]
pub async fn remove_tag(
    _guard: Auth<BookAdmin>,
    tag_id: TagGuard,
    book_id: BookGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let tag = tag_id.to_tag(&conn).await.into_flash(uri!("/"))?;
    let tag_cloned = tag.clone();
    conn.run(move |c| {
        TagMappingFinder::new(c, None)
            .product(&book.book_id)
            .tag(&tag)
            .first()
            .map(|x| x.delete(c))
    })
    .await
    .into_flash(uri!("/"))?
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_tag(tag_cloned.get_id()))))
}

#[get("/add_tag?<tag_id>&<book_id>")]
pub async fn add_tag(
    _guard: Auth<BookAdmin>,
    tag_id: TagGuard,
    book_id: BookGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let tag = tag_id.to_tag(&conn).await.into_flash(uri!("/"))?;
    let tag_cloned = tag.clone();
    conn.run(move |c| TagMapping::create(c, &tag, &book.book_id))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_tag(tag_cloned.get_id()))))
}

#[derive(Template)]
#[template(path = "admin/tag.html")]
pub struct AdminTagPage {
    tag: Tag,
    tagged: Vec<ProductInfo>,
    untagged: Vec<ProductInfo>,
}

#[get("/tag?<id>")]
pub async fn admin_tag(
    _guard: Auth<BookAdmin>,
    id: TagGuard,
    conn: DbConn,
) -> Result<AdminTagPage, Flash<Redirect>> {
    let id = id.to_tag(&conn).await.into_flash(uri!("/"))?;
    let tag = id.clone();
    let tag_clone = id.clone();
    let tagged = conn
        .run(move |c| {
            ProductFinder::list_info(c).map(|x| {
                x.into_iter()
                    .filter(|p| TagMappingFinder::has_mapping(c, &tag, &p.to_id()).unwrap_or(false))
                    .collect()
            })
        })
        .await
        .into_flash(uri!("/"))?;

    let untagged = conn
        .run(move |c| {
            ProductFinder::list_info(c).map(|x| {
                x.into_iter()
                    .filter(|p| {
                        !TagMappingFinder::has_mapping(c, &tag_clone, &p.to_id()).unwrap_or(false)
                    })
                    .collect()
            })
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(AdminTagPage {
        tag: id,
        tagged,
        untagged,
    })
}

#[derive(Template)]
#[template(path = "admin/books.html")]
pub struct AdminBooksPage {
    normal_books: Vec<ProductInfo>,
    verified_books: Vec<ProductInfo>,
    disabled_books: Vec<ProductInfo>,
}

// If the user has already been verified, show him the root dashboard
#[get("/books")]
pub async fn admin_books(
    _guard: Auth<BookAdmin>,
    conn: DbConn,
) -> Result<AdminBooksPage, Flash<Redirect>> {
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

    Ok(AdminBooksPage {
        normal_books,
        disabled_books,
        verified_books,
    })
}

#[get("/verify_book?<book_id>")]
pub async fn verify_book(
    _guard: Auth<BookAdmin>,
    book_id: BookGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| {
        book.book_info
            .set_product_status(ProductStatus::Verified)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_books)))
}

#[get("/disable_book?<book_id>")]
pub async fn disable_book(
    _guard: Auth<BookAdmin>,
    book_id: BookGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| {
        book.book_info
            .set_product_status(ProductStatus::Disabled)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_books)))
}

#[get("/normalize_book?<book_id>")]
pub async fn normalize_book(
    _guard: Auth<BookAdmin>,
    book_id: BookGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| {
        book.book_info
            .set_product_status(ProductStatus::Normal)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_books)))
}

#[get("/refund_order?<order_id>")]
pub async fn refund_order(
    _auth: Auth<OrderRefundable>,
    order_id: OrderGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<Redirect, Flash<Redirect>> {
    let id = order_id.to_id(&conn).await.into_flash(uri!("/"))?;
    loop {
        let resp = client
            .request(priv_key, CancelTrade::new(id.id.get_id()))
            .into_flash(uri!("/"))?
            .send::<CancelTradeResp>()
            .await
            .into_flash(uri!("/"))?
            .into_flash(uri!("/"))?;
        if resp.retry_flag == "N" {
            break;
        }
    }

    conn.run(move |c| id.id.refund(c))
        .await
        .into_flash(uri!("/admin", admin_orders))?;
    Ok(Redirect::to(uri!("/admin", admin_orders)))
}

#[get("/finish_order?<order_id>")]
pub async fn finish_order(
    _auth: Auth<OrderFinishable>,
    order_id: OrderGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let info = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| {
        info.order_info
            .set_transaction_status(TransactionStatus::Finished)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_orders)))
}

#[get("/")]
pub async fn admin(_guard: Auth<BookAdmin>) -> Redirect {
    Redirect::to(uri!("/admin", admin_metrics))
}
