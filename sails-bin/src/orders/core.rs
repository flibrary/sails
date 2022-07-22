use crate::{
    guards::*,
    utils::{i18n::I18n, telegram_bot::TelegramBot},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    form::{Form, Strict},
    response::{Flash, Redirect},
    State,
};
use sails_db::{enums::Payment, products::*, transactions::*};
use std::num::NonZeroU32;

#[derive(FromForm)]
pub struct CheckoutInfo {
    quantity: NonZeroU32,
    address: String,
    payment: Payment,
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
                info.payment.clone(),
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

    Ok(Redirect::to(uri!("/orders", super::order_info_alipay(id))))
}

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
