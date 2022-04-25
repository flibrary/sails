use crate::{
    alipay::{
        AlipayAppPrivKey, AlipayClient, CancelTrade, CancelTradeResp, Precreate, PrecreateResp,
        RefundTrade, RefundTradeResp, SignedResponse, TradeQuery, TradeQueryResp,
    },
    guards::*,
    telegram_bot::TelegramBot,
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    form::{Form, Strict},
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    enums::TransactionStatus, error::SailsDbError, products::*, tags::TagMappingFinder,
    transactions::*,
};
use std::num::NonZeroU32;

#[derive(Template)]
#[template(path = "orders/order_info_seller.html")]
pub struct OrderInfoSeller {
    prod: ProductInfo,
    order: TransactionInfo,
}

#[get("/order_info?<order_id>", rank = 2)]
pub async fn order_info_seller(
    _auth: Auth<OrderReadable>,
    order_id: OrderGuard,
    conn: DbConn,
) -> Result<OrderInfoSeller, Flash<Redirect>> {
    let order = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(OrderInfoSeller {
        prod: order.prod_info,
        order: order.order_info,
    })
}

#[derive(Template)]
#[template(path = "orders/order_info.html")]
pub struct OrderInfoBuyer {
    prod: ProductInfo,
    order: TransactionInfo,
    // Alipay precreate API response
    resp: Option<Result<PrecreateResp, SignedResponse<PrecreateResp>>>,
}

#[get("/order_info?<order_id>", rank = 1)]
pub async fn order_info_buyer(
    // This page contains progressable information
    // TODO: this is not a good enough distinguishment
    _auth: Auth<OrderProgressable>,
    order_id: OrderGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<OrderInfoBuyer, Flash<Redirect>> {
    let order = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
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
                    order.prod_info.get_prodname(),
                    order.order_info.get_total(),
                ),
            )
            .into_flash(uri!("/"))?
            .send::<PrecreateResp>(client.client())
            .await
            .into_flash(uri!("/"))?;
        Ok(OrderInfoBuyer {
            prod: order.prod_info,
            order: order.order_info,
            resp: Some(resp),
        })
    } else {
        Ok(OrderInfoBuyer {
            prod: order.prod_info,
            order: order.order_info,
            resp: None,
        })
    }
}

#[get("/cancel_order?<order_id>&<redirect>")]
pub async fn cancel_order(
    order_id: OrderGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
    redirect: String,
    bot: &State<TelegramBot>,
) -> Result<Redirect, Flash<Redirect>> {
    let info = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    let status = info.order_info.get_transaction_status();
    // We only allow users to cancel their orders if they have not finished them.
    match status {
        TransactionStatus::Placed => {
            loop {
                let resp = client
                    .request(priv_key, CancelTrade::new(info.order_info.get_id()))
                    .into_flash(uri!("/"))?
                    .send::<CancelTradeResp>(client.client())
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
        }
        TransactionStatus::Paid => {
            client
                .request(
                    priv_key,
                    RefundTrade::new(
                        info.order_info.get_id(),
                        "用户发起无理由退款",
                        info.order_info.get_total(),
                    ),
                )
                .into_flash(uri!("/"))?
                .send::<RefundTradeResp>(client.client())
                .await
                .into_flash(uri!("/"))?
                .into_flash(uri!("/"))?;

            conn.run(move |c| info.order_info.refund(c))
                .await
                .into_flash(uri!("/"))?;
        }
        _ => {
            return Err(Flash::error(
                Redirect::to(uri!("/")),
                "refunds not allowed due to order status constraints",
            ))
        }
    }

    bot.send_order_update(order_id.get_id(), &conn)
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(redirect))
}

// Basically, we syncronize our trade status with that in alipay
#[get("/progress?<order_id>", rank = 1)]
pub async fn progress(
    _auth: Auth<OrderProgressable>,
    order_id: OrderGuard,
    db: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
    bot: &State<TelegramBot>,
) -> Result<Redirect, Flash<Redirect>> {
    let order = order_id.to_info(&db).await.into_flash(uri!("/"))?;

    let resp = client
        .request(priv_key, TradeQuery::new(order.order_info.get_id()))
        .into_flash(uri!("/"))?
        .send::<TradeQueryResp>(client.client())
        .await
        .into_flash(uri!("/"))?
        .into_flash(uri!("/"))?;

    let prod_id = order.prod_info.to_id();
    let digicon: bool = db
        .run(move |c| -> Result<bool, SailsDbError> {
            let tags = TagMappingFinder::new(c, None)
                .product(&prod_id)
                .search_tag()?;
            Ok(tags.iter().any(|x| x.get_id() == "digicon"))
        })
        .await
        .into_flash(uri!("/"))?;

    let status = match resp.trade_status.as_str() {
        // Both of these indicate that we have successfully finished the transaction.
        // TRADE_FINISHED indicates it has been well pass the refunding deadline.
        "TRADE_SUCCESS" | "TRADE_FINISHED" => {
            if !digicon {
                TransactionStatus::Paid
            } else {
                TransactionStatus::Finished
            }
        }
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

    bot.send_order_update(order_id.get_id(), &db)
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/orders", order_info_buyer(order_id))))
}

#[derive(Template)]
#[template(path = "orders/checkout.html")]
pub struct CheckoutPage {
    prod: ProductInfo,
    recent_address: Option<String>,
}

#[get("/checkout?<prod_id>")]
pub async fn checkout(
    db: DbConn,
    prod_id: ProdGuard,
    user: UserIdGuard<Cookie>,
) -> Result<CheckoutPage, Flash<Redirect>> {
    let addr = db
        .run(move |c| TransactionFinder::most_recent_order(c, &user.id))
        .await
        .map(|x| x.get_address().to_string())
        .ok();
    Ok(CheckoutPage {
        prod: prod_id.to_info(&db).await.into_flash(uri!("/"))?.prod_info,
        recent_address: addr,
    })
}

#[derive(FromForm)]
pub struct CheckoutInfo {
    quantity: NonZeroU32,
    address: String,
}

#[post("/purchase?<prod_id>", data = "<info>")]
pub async fn purchase(
    db: DbConn,
    prod_id: ProdGuard,
    user: UserInfoGuard<Cookie>,
    info: Form<Strict<CheckoutInfo>>,
    bot: &State<TelegramBot>,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_info(&db).await.into_flash(uri!("/"))?;

    let info = db
        // TODO: We need to allow user to specify quantity
        .run(move |c| {
            Transactions::buy(
                c,
                &prod.prod_info.to_id(),
                &user.info.to_id(),
                info.quantity.get(),
                &info.address,
            )
            .map(|t| t.get_info(c))
        })
        .await
        .into_flash(uri!("/"))?
        .into_flash(uri!("/"))?;

    // TODO: can we make it elegant
    let id = info.get_id().to_string();

    bot.send_order_update(&id, &db)
        .await
        .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/orders", order_info_buyer(id))))
}
