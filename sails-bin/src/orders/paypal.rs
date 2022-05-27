use crate::{guards::*, i18n::I18n, telegram_bot::TelegramBot, DbConn, IntoFlash};
use askama::Template;
use paypal_rs::{
    api::orders::*,
    data::orders::{Order, *},
    Client, HeaderParams,
};
use rocket::{
    http::Status,
    response::{Flash, Redirect},
    serde::json::Json,
    State,
};
use sails_db::{
    digicons::*, enums::TransactionStatus, error::SailsDbError, products::*, transactions::*,
};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct PaypalAuth {
    paypal_client_id: String,
    paypal_secret: String,
}

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
        client_id: paypal_auth.paypal_client_id.clone(),
    })
}

#[derive(Deserialize)]
pub struct CapturePaypalOrder {
    paypal_order_id: String,
}

#[post(
    "/capture_paypal_order?<order_id>",
    format = "json",
    data = "<paypal_info>"
)]
pub async fn capture_paypal_order(
    _auth: Auth<OrderProgressable>,
    paypal_auth: &State<PaypalAuth>,
    order_id: OrderGuard,
    paypal_info: Json<CapturePaypalOrder>,
    conn: DbConn,
) -> Result<Json<Order>, Status> {
    let info = order_id
        .to_info(&conn)
        .await
        .map_err(|_| Status::new(500))?;

    let prod_id = info.prod_info.to_id();
    let digicon: bool = conn
        .run(move |c| -> Result<bool, SailsDbError> {
            DigiconMappingFinder::new(c, None)
                .product(&prod_id)
                .count()
                .map(|x| x > 0)
        })
        .await
        .map_err(|_| Status::new(500))?;

    let mut client = Client::new(
        paypal_auth.paypal_client_id.clone(),
        paypal_auth.paypal_secret.clone(),
        false, // TODO: change this to false in production
    );

    client
        .get_access_token()
        .await
        .map_err(|_| Status::new(502))?;

    let capture = CaptureOrder::new(&paypal_info.paypal_order_id);

    // Without the body, reqwest doesn't automatically append needed header.
    let mut header = HeaderParams::default();
    header.content_type = Some("application/json".to_string());

    let resp = client
        .execute_ext(&capture, header)
        .await
        .map_err(|_| Status::new(502))?;

    // If we have got the money, record it
    if resp.status == OrderStatus::Completed {
        let status = if digicon {
            TransactionStatus::Finished
        } else {
            TransactionStatus::Paid
        };

        conn.run(move |c| info.order_info.set_transaction_status(status).update(c))
            .await
            .map_err(|_| Status::new(500))?;
    }

    Ok(Json(resp))
}

#[post("/create_paypal_order?<order_id>")]
pub async fn create_paypal_order(
    _guard: Auth<OrderProgressable>,
    paypal_auth: &State<PaypalAuth>,
    order_id: OrderGuard,
    conn: DbConn,
) -> Result<Json<Order>, Status> {
    let info = order_id
        .to_info(&conn)
        .await
        // database error: internal server error
        .map_err(|_| Status::new(500))?;

    let mut client = Client::new(
        paypal_auth.paypal_client_id.clone(),
        paypal_auth.paypal_secret.clone(),
        true, // TODO: change this to false in production
    );

    client
        .get_access_token()
        .await
        .map_err(|_| Status::new(502))?;

    let order = OrderPayloadBuilder::default()
        .intent(Intent::Capture)
        .purchase_units(vec![PurchaseUnit::new(Amount::new(
            info.order_info.get_currency().into(),
            &info.order_info.get_total().to_string(),
        ))])
        .build()
        //        .map_err(|_| Status::new(502))?;
        .unwrap();

    let create_order = CreateOrder::new(order);

    let resp = client
        .execute(&create_order)
        .await
        //        .map_err(|_| Status::new(502))?;
        .unwrap();

    Ok(Json(resp))
}

#[get("/cancel_order?<order_id>", rank = 2)]
pub async fn cancel_order_paypal(
    _is_paypal: Auth<OrderWithPaypal>,
    _auth: Auth<OrderProgressable>,
    order_id: OrderGuard,
    conn: DbConn,
    bot: &State<TelegramBot>,
) -> Result<Redirect, Flash<Redirect>> {
    let info = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    let status = info.order_info.get_transaction_status();
    // We only allow users to cancel their orders if they have not finished them.
    match status {
        TransactionStatus::Placed => {
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
    Ok(Redirect::to(uri!("/orders", order_info_paypal(order_id))))
}
