mod alipay;
mod paypal;

pub use alipay::*;
pub use paypal::*;

use crate::{
    infras::{guards::*, i18n::I18n},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{products::*, transactions::*};

#[derive(Template)]
#[template(path = "orders/checkout.html")]
pub struct CheckoutPage {
    i18n: I18n,
    prod: ProductInfo,
    recent_address: Option<String>,
}

#[get("/checkout?<prod_id>")]
pub async fn checkout(
    i18n: I18n,
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
        i18n,
        prod: prod_id.to_info(&db).await.into_flash(uri!("/"))?.prod_info,
        recent_address: addr,
    })
}

#[derive(Template)]
#[template(path = "orders/order_info_seller.html")]
pub struct OrderInfoSeller {
    i18n: I18n,
    prod: ProductInfo,
    order: TransactionInfo,
}

#[get("/order_info?<order_id>", rank = 3)]
pub async fn order_info_seller(
    i18n: I18n,
    _auth: Auth<OrderReadable>,
    order_id: OrderGuard,
    conn: DbConn,
) -> Result<OrderInfoSeller, Flash<Redirect>> {
    let order = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(OrderInfoSeller {
        i18n,
        prod: order.prod_info,
        order: order.order_info,
    })
}
