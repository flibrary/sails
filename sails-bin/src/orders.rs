use crate::{
    alipay::{
        AlipayAppPrivKey, AlipayClient, Precreate, PrecreateResp, SignedResponse, TradeQuery,
        TradeQueryResp,
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
#[template(path = "orders/order_info.html")]
pub struct OrderInfo {
    book: ProductInfo,
    order: TransactionInfo,
    // Alipay precreate API response
    resp: Option<Result<PrecreateResp, SignedResponse<PrecreateResp>>>,
}

#[get("/order_info")]
pub async fn order_info(
    _buyer: Role<Buyer>,
    order: OrderInfoGuard,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<OrderInfo, Flash<Redirect>> {
    if order.order_info.get_transaction_status() == &TransactionStatus::Placed {
        // It seems like we could request precreation even if the user has already paid the bill or the trade has already been created.
        // If, in the future, this behavior changes, we have to come up with a better mechanism.
        // Currently, if anything goes wrong, we would have the message for debug, and the confirm button would still be available.
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
            .into_flash(uri!("/user", crate::user::portal))?
            .send::<PrecreateResp>()
            .await
            .into_flash(uri!("/user", crate::user::portal))?;
        Ok(OrderInfo {
            book: order.book_info,
            order: order.order_info,
            resp: Some(resp),
        })
    } else {
        Ok(OrderInfo {
            book: order.book_info,
            order: order.order_info,
            resp: None,
        })
    }
}

#[get("/confirm")]
pub async fn confirm(
    _role: Role<Buyer>,
    order: OrderInfoGuard,
    db: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<Redirect, Flash<Redirect>> {
    let id = order.order_info.get_id().to_string();

    let resp = client
        .request(priv_key, TradeQuery::new(order.order_info.get_id()))
        .into_flash(uri!("/user", crate::user::portal))?
        .send::<TradeQueryResp>()
        .await
        .into_flash(uri!("/user", crate::user::portal))?
        .into_flash(uri!("/user", crate::user::portal))?;

    // Both of these indicate that we have successfully finished the transaction.
    // TRADE_FINISHED indicates it has been well pass the refunding deadline.
    if (resp.trade_status == "TRADE_SUCCESS") || (resp.trade_status == "TRADE_FINISHED") {
        // We only allow confirmation on placed products
        if order.order_info.get_transaction_status() == &TransactionStatus::Placed {
            db.run(move |c| {
                order
                    .order_info
                    .set_transaction_status(TransactionStatus::Paid)
                    .update(c)
            })
            .await
            .into_flash(uri!("/user", crate::user::portal))?;
        }
    }
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
        .into_flash(uri!("/user", crate::user::portal))?;

    Ok(Redirect::to(format!(
        "/orders/order_info?order_id={}",
        id.get_id()
    )))
}
