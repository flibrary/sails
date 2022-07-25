use crate::{
    infras::{guards::*, i18n::I18n},
    services::orders::PaypalAuth,
    DbConn, IntoFlash,
};
use askama::Template;

use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{products::*, transactions::*};

#[derive(Template)]
#[template(path = "orders/order_info_paypal.html")]
pub struct OrderInfoBuyerPaypal {
    i18n: I18n,
    prod: ProductInfo,
    order: TransactionInfo,
    client_id: String,
}

#[get("/order_info?<order_id>", rank = 2)]
pub async fn order_info_paypal(
    i18n: I18n,
    _is_alipay: Auth<OrderWithPaypal>,
    // This page contains progressable information
    // TODO: this is not a good enough distinguishment
    _auth: Auth<OrderProgressable>,
    order_id: OrderGuard,
    conn: DbConn,
    paypal_auth: &State<PaypalAuth>,
) -> Result<OrderInfoBuyerPaypal, Flash<Redirect>> {
    let order = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(OrderInfoBuyerPaypal {
        i18n,
        prod: order.prod_info,
        order: order.order_info,
        client_id: paypal_auth.client_id.clone(),
    })
}
