use crate::{guards::*, DbConn, IntoFlash};
use askama::Template;
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{enums::TransactionStatus, products::*, transactions::*};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlipayId {
    alipay_id: String,
}

#[derive(Template)]
#[template(path = "orders/alipay_process.html")]
pub struct AlipayProcess {
    book: ProductInfo,
    order: TransactionInfo,
    alipay_id: String,
}

#[get("/alipay")]
pub async fn alipay_order_process(
    order: OrderInfoGuard,
    alipay_id: &State<AlipayId>,
) -> AlipayProcess {
    AlipayProcess {
        book: order.book_info,
        order: order.order_info,
        alipay_id: alipay_id.alipay_id.to_string(),
    }
}

#[derive(Template)]
#[template(path = "orders/admin_order_info.html")]
pub struct AdminOrderInfo {
    book: ProductInfo,
    order: TransactionInfo,
}

#[get("/admin_order_info")]
pub async fn admin_order_info(_guard: Role<Admin>, order: OrderInfoGuard) -> AdminOrderInfo {
    AdminOrderInfo {
        book: order.book_info,
        order: order.order_info,
    }
}

#[derive(Template)]
#[template(path = "orders/order_info_buyer.html")]
pub struct OrderInfoBuyer {
    book: ProductInfo,
    order: TransactionInfo,
}

#[get("/order_info", rank = 2)]
pub async fn order_info_buyer(_buyer: Role<Buyer>, order: OrderInfoGuard) -> OrderInfoBuyer {
    OrderInfoBuyer {
        book: order.book_info,
        order: order.order_info,
    }
}

#[derive(Template)]
#[template(path = "orders/order_info_seller.html")]
pub struct OrderInfoSeller {
    book: ProductInfo,
    order: TransactionInfo,
}

#[get("/order_info", rank = 1)]
pub async fn order_info_seller(_seller: Role<Seller>, order: OrderInfoGuard) -> OrderInfoSeller {
    OrderInfoSeller {
        book: order.book_info,
        order: order.order_info,
    }
}

#[get("/confirm")]
pub async fn confirm(
    _seller: Role<Seller>,
    order: OrderInfoGuard,
    db: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let id = order.order_info.get_id().to_string();
    // We only allow confirmation on placed products
    if order.order_info.get_transaction_status() == &TransactionStatus::Placed {
        db.run(move |c| {
            order
                .order_info
                .set_transaction_status(TransactionStatus::Paid)
                .update(c)
        })
        .await
        .into_flash(uri!("/market", crate::market::market))?;
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
        .into_flash(uri!("/market", crate::market::market))?;

    Ok(Redirect::to(format!(
        "/orders/order_info?order_id={}",
        id.get_id()
    )))
}
