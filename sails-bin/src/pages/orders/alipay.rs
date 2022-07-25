use crate::{
    infras::{
        alipay::{AlipayAppPrivKey, AlipayClient, Precreate, PrecreateResp, SignedResponse},
        guards::*,
        i18n::I18n,
    },
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{enums::TransactionStatus, products::*, transactions::*};

#[derive(Template)]
#[template(path = "orders/order_info_alipay.html")]
pub struct OrderInfoBuyerAlipay {
    i18n: I18n,
    prod: ProductInfo,
    order: TransactionInfo,
    // Alipay precreate API response
    resp: Option<Result<PrecreateResp, SignedResponse<PrecreateResp>>>,
}

#[get("/order_info?<order_id>", rank = 1)]
pub async fn order_info_alipay(
    i18n: I18n,
    _is_alipay: Auth<OrderWithAlipay>,
    // This page contains progressable information
    // TODO: this is not a good enough distinguishment
    _auth: Auth<OrderProgressable>,
    order_id: OrderGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<OrderInfoBuyerAlipay, Flash<Redirect>> {
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
        Ok(OrderInfoBuyerAlipay {
            i18n,
            prod: order.prod_info,
            order: order.order_info,
            resp: Some(resp),
        })
    } else {
        Ok(OrderInfoBuyerAlipay {
            i18n,
            prod: order.prod_info,
            order: order.order_info,
            resp: None,
        })
    }
}
