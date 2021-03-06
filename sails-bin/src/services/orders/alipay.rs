use crate::{
    infras::{
        alipay::{
            AlipayAppPrivKey, AlipayClient, CancelTrade, CancelTradeResp, RefundTrade,
            RefundTradeResp, TradeQuery, TradeQueryResp,
        },
        guards::*,
        tg_bot::TelegramBot,
    },
    pages::orders::*,
    DbConn, IntoFlash,
};
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{digicons::*, enums::TransactionStatus, error::SailsDbError};

#[get("/cancel_order?<order_id>", rank = 1)]
pub async fn cancel_order_alipay(
    _is_alipay: Auth<OrderWithAlipay>,
    _auth: Auth<OrderProgressable>,
    order_id: OrderGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
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
    Ok(Redirect::to(uri!("/orders", order_info_alipay(order_id))))
}

// Basically, we syncronize our trade status with that in alipay
#[get("/progress?<order_id>", rank = 1)]
pub async fn progress_alipay(
    _auth: Auth<OrderProgressable>,
    _is_alipay: Auth<OrderWithAlipay>,
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
            DigiconMappingFinder::new(c, None)
                .product(&prod_id)
                .count()
                .map(|x| x > 0)
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
    Ok(Redirect::to(uri!("/orders", order_info_alipay(order_id))))
}
