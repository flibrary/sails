use crate::{
    infras::{guards::*, tg_bot::TelegramBot},
    pages::orders::*,
    DbConn, IntoFlash,
};
use rocket::{
    form::{Form, Strict},
    response::{Flash, Redirect},
    State,
};
use sails_db::{enums::Payment, transactions::*};
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

    Ok(Redirect::to(uri!("/orders", order_info_alipay(id))))
}
