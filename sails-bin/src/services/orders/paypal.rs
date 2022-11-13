use crate::{
    infras::{guards::*, tg_bot::TelegramBot},
    pages::orders::*,
    DbConn, IntoFlash,
};
use paypal_rs::{
    api::orders::*,
    client::PaypalEnv,
    data::orders::{Order, *},
    Client, HeaderParams,
};
use rocket::{
    http::Status,
    response::{Flash, Redirect},
    serde::json::Json,
    State,
};
use sails_db::{digicons::*, enums::TransactionStatus, error::SailsDbError};
use serde::Deserialize;

// This is considered appropriate in service as it is information only, not quite an infrastructure.
#[derive(Clone, Debug, Deserialize)]
pub struct PaypalAuth {
    pub client_id: String,
    pub secret: String,
}

#[get("/progress?<order_id>", rank = 2)]
pub async fn progress_paypal(
    _auth: Auth<OrderProgressable>,
    _is_paypal: Auth<OrderWithPaypal>,
    order_id: OrderGuard,
    paypal_auth: &State<PaypalAuth>,
    conn: DbConn,
    bot: &State<TelegramBot>,
) -> Result<Redirect, Flash<Redirect>> {
    let order = order_id.to_info(&conn).await.into_flash(uri!("/"))?;

    let paypal_order_id = order
        .order_info
        .get_payment_detail()
        .ok_or("PayPal payment detail not found")
        .into_flash(uri!("/"))?;

    let mut client = Client::new(
        paypal_auth.client_id.clone(),
        paypal_auth.secret.clone(),
        #[cfg(debug_assertions)]
        PaypalEnv::Sandbox,
        #[cfg(not(debug_assertions))]
        PaypalEnv::Live,
    );

    client.get_access_token().await.into_flash(uri!("/"))?;

    let prod_id = order.prod_info.to_id();
    let digicon: bool = conn
        .run(move |c| -> Result<bool, SailsDbError> {
            DigiconMappingFinder::new(c, None)
                .product(&prod_id)
                .count()
                .map(|x| x > 0)
        })
        .await
        .into_flash(uri!("/"))?;

    // Without the body, reqwest doesn't automatically append needed header.
    let header = HeaderParams {
        content_type: Some("application/json".to_string()),
        ..Default::default()
    };

    let resp = client
        .execute_ext(&ShowOrderDetails::new(paypal_order_id), header.clone())
        .await
        .into_flash(uri!("/"))?;

    let status = match resp.status {
        // If the preliminary order status indicates order is in progress, try to capture it
        OrderStatus::Approved | OrderStatus::Saved => {
            let resp = client
                .execute_ext(&CaptureOrder::new(paypal_order_id), header.clone())
                .await
                .into_flash(uri!("/"))?;
            // Use the new order status
            resp.status
        }
        // We don't need to capture and update the paypal order status
        _ => resp.status,
    };

    // Map paypal order status to ours
    let status = match status {
        // We have successfully finished the transaction.
        OrderStatus::Completed => {
            if !digicon {
                TransactionStatus::Paid
            } else {
                TransactionStatus::Finished
            }
        }
        // Trade has been closed,
        OrderStatus::Voided => TransactionStatus::Refunded,
        // Still not captured
        _ => TransactionStatus::Placed,
    };
    conn.run(move |c| order.order_info.set_transaction_status(status).update(c))
        .await
        .into_flash(uri!("/"))?;

    bot.send_order_update(order_id.get_id(), &conn)
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/orders", order_info_paypal(order_id))))
}

#[post("/capture_paypal_order?<order_id>")]
pub async fn capture_paypal_order(
    _auth: Auth<OrderProgressable>,
    paypal_auth: &State<PaypalAuth>,
    order_id: OrderGuard,
    conn: DbConn,
    bot: &State<TelegramBot>,
) -> Result<Json<Order>, Status> {
    let info = order_id
        .to_info(&conn)
        .await
        .map_err(|_| Status::new(500))?;

    let paypal_order_id = info.order_info.get_payment_detail().ok_or_else(|| {
        error_!(
            "PayPal payment detail not found for order {}",
            info.order_info.get_id()
        );
        Status::new(405)
    })?;

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
        paypal_auth.client_id.clone(),
        paypal_auth.secret.clone(),
        #[cfg(debug_assertions)]
        PaypalEnv::Sandbox,
        #[cfg(not(debug_assertions))]
        PaypalEnv::Live,
    );

    client.get_access_token().await.map_err(|e| {
        error_!("failed to generate PayPal access token: {}", e);
        Status::new(502)
    })?;

    let capture = CaptureOrder::new(paypal_order_id);

    // Without the body, reqwest doesn't automatically append needed header.
    let header = HeaderParams {
        content_type: Some("application/json".to_string()),
        ..Default::default()
    };

    let resp = client.execute_ext(&capture, header).await.map_err(|e| {
        error_!("failed to capture PayPal order: {}", e);
        Status::new(502)
    })?;

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

    bot.send_order_update(order_id.get_id(), &conn)
        .await
        .map_err(|_| Status::new(502))?;

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
        paypal_auth.client_id.clone(),
        paypal_auth.secret.clone(),
        #[cfg(debug_assertions)]
        PaypalEnv::Sandbox,
        #[cfg(not(debug_assertions))]
        PaypalEnv::Live,
    );

    client.get_access_token().await.map_err(|e| {
        error_!("failed to generate PayPal access token: {}", e);
        Status::new(502)
    })?;

    let order = OrderPayloadBuilder::default()
        .intent(Intent::Capture)
        .purchase_units(vec![PurchaseUnit::new(Amount::new(
            info.order_info.get_currency().into(),
            &info.order_info.get_total().to_string(),
        ))])
        .build()
        .map_err(|e| {
            error_!("failed to build PayPal order payload: {}", e);
            Status::new(502)
        })?;

    let create_order = CreateOrder::new(order);

    let resp = client.execute(&create_order).await.map_err(|e| {
        error_!("failed to create PayPal order: {}", e);
        Status::new(502)
    })?;

    let paypal_order_id = resp.id.clone();
    conn.run(move |c| {
        info.order_info
            .set_payment_detail(Some(paypal_order_id))
            .update(c)
    })
    .await
    .map_err(|_| Status::new(500))?;

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
