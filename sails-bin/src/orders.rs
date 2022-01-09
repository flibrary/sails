use crate::{
    alipay::{
        AlipayAppPrivKey, AlipayClient, CancelTrade, CancelTradeResp, Precreate, PrecreateResp,
        SignedResponse, TradeQuery, TradeQueryResp,
    },
    guards::*,
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{enums::TransactionStatus, products::*, transactions::*};

#[derive(Template)]
#[template(path = "orders/order_info_seller_or_admin.html")]
pub struct OrderInfoSellerOrAdmin {
    book: ProductInfo,
    order: TransactionInfo,
}

#[get("/order_info", rank = 2)]
pub async fn order_info_seller(
    _auth: Auth<OrderReadable>,
    order: OrderInfoGuard,
) -> OrderInfoSellerOrAdmin {
    OrderInfoSellerOrAdmin {
        book: order.book_info,
        order: order.order_info,
    }
}

#[derive(Template)]
#[template(path = "orders/order_info.html")]
pub struct OrderInfoBuyer {
    book: ProductInfo,
    order: TransactionInfo,
    // Alipay precreate API response
    resp: Option<Result<PrecreateResp, SignedResponse<PrecreateResp>>>,
}

#[get("/order_info", rank = 1)]
pub async fn order_info_buyer(
    // This page contains progressable information
    // TODO: this is not a good enough distinguishment
    _auth: Auth<OrderProgressable>,
    order: OrderInfoGuard,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<OrderInfoBuyer, Flash<Redirect>> {
    if order.order_info.get_transaction_status() == &TransactionStatus::Placed {
        // It seems like we could request precreation even if the user has already paid the bill or the trade has already been created.
        // If, in the future, this behavior changes, we have to come up with a better mechanism.
        // Currently, if anything goes wrong, we would have the message for debug, and the next button would still be available.
        let resp = client
            .request(
                priv_key,
                Precreate::new(
                    order.order_info.get_id(),
                    // Alipay doesn't play well with UTF-8
                    order.book_info.get_prodname(),
                    order.book_info.get_price(),
                ),
            )
            .into_flash(uri!("/"))?
            .send::<PrecreateResp>()
            .await
            .into_flash(uri!("/"))?;
        Ok(OrderInfoBuyer {
            book: order.book_info,
            order: order.order_info,
            resp: Some(resp),
        })
    } else {
        Ok(OrderInfoBuyer {
            book: order.book_info,
            order: order.order_info,
            resp: None,
        })
    }
}

#[get("/cancel_order")]
pub async fn user_cancel_order(
    // Note: here we set the auth flag to order progressable, meaning that user or customer service can cancel order in limited situations.
    _auth: Auth<OrderProgressable>,
    info: OrderInfoGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<Redirect, Flash<Redirect>> {
    let status = info.order_info.get_transaction_status();
    // We only allow users to cancel their orders if they have not finished them.
    if (status == &TransactionStatus::Placed) || (status == &TransactionStatus::Paid) {
        loop {
            let resp = client
                .request(priv_key, CancelTrade::new(info.order_info.get_id()))
                .into_flash(uri!("/"))?
                .send::<CancelTradeResp>()
                .await
                .into_flash(uri!("/"))?
                .into_flash(uri!("/"))?;
            if resp.retry_flag == "N" {
                break;
            }
        }

        conn.run(move |c| info.order_info.refund(c))
            .await
            .into_flash(uri!("/"))?;
        Ok(Redirect::to(uri!("/user", crate::user::portal)))
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/")),
            "refunds not allowed due to order status constraints",
        ))
    }
}

// Basically, we syncronize our trade status with that in alipay
#[get("/progress", rank = 1)]
pub async fn progress(
    _auth: Auth<OrderProgressable>,
    order: OrderInfoGuard,
    db: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<Redirect, Flash<Redirect>> {
    let id = order.order_info.get_id().to_string();

    let resp = client
        .request(priv_key, TradeQuery::new(order.order_info.get_id()))
        .into_flash(uri!("/"))?
        .send::<TradeQueryResp>()
        .await
        .into_flash(uri!("/"))?
        .into_flash(uri!("/"))?;

    let status = match resp.trade_status.as_str() {
        // Both of these indicate that we have successfully finished the transaction.
        // TRADE_FINISHED indicates it has been well pass the refunding deadline.
        "TRADE_SUCCESS" | "TRADE_FINISHED" => TransactionStatus::Paid,
        // Trade has been closed,
        "TRADE_CLOSED" => TransactionStatus::Refunded,
        "WAIT_BUYER_PAY" => TransactionStatus::Placed,
        // This should NEVER happen
        other_status => {
            return Err(Flash::error(
                Redirect::to(uri!("/")),
                format!("unexpected alipay trade_status: {}", other_status),
            ))
        }
    };
    db.run(move |c| order.order_info.set_transaction_status(status).update(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(format!("/orders/order_info?order_id={}", id)))
}

#[get("/purchase")]
pub async fn purchase(
    db: DbConn,
    book: BookIdGuard,
    user: UserIdGuard<Cookie>,
) -> Result<Redirect, Flash<Redirect>> {
    let id = db
        .run(move |c| Transactions::buy(c, &book.book_id, &user.id))
        .await
        .into_flash(uri!("/"))?;

    Ok(Redirect::to(format!(
        "/orders/order_info?order_id={}",
        id.get_id()
    )))
}
