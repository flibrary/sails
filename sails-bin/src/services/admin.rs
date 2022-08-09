use crate::{
    infras::{
        alipay::{AlipayAppPrivKey, AlipayClient, RefundTrade, RefundTradeResp},
        guards::*,
    },
    pages::admin::*,
    DbConn, IntoFlash,
};
use rocket::{
    form::Form,
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    coupons::*,
    enums::{ProductStatus, TransactionStatus},
    tags::*,
};

#[get("/remove_tag?<tag_id>&<prod_id>")]
pub async fn remove_tag(
    _guard: Auth<TagWritable>,
    tag_id: TagGuard,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let tag = tag_id.to_tag(&conn).await.into_flash(uri!("/"))?;
    let tag_cloned = tag.clone();
    conn.run(move |c| {
        TagMappingFinder::new(c, None)
            .product(&prod.prod_id)
            .tag(&tag)
            .first()
            .map(|x| x.delete(c))
    })
    .await
    .into_flash(uri!("/"))?
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_tag(tag_cloned.get_id()))))
}

#[get("/add_tag?<tag_id>&<prod_id>")]
pub async fn add_tag(
    _guard: Auth<TagWritable>,
    tag_id: TagGuard,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let tag = tag_id.to_tag(&conn).await.into_flash(uri!("/"))?;
    let tag_cloned = tag.clone();
    conn.run(move |c| TagMapping::create(c, &tag, &prod.prod_id))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_tag(tag_cloned.get_id()))))
}

#[get("/verify_prod?<prod_id>")]
pub async fn verify_prod(
    _guard: Auth<ProdAdmin>,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_info(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| {
        prod.prod_info
            .set_product_status(ProductStatus::Verified)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_prods)))
}

#[get("/disable_prod?<prod_id>")]
pub async fn disable_prod(
    _guard: Auth<ProdAdmin>,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_info(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| {
        prod.prod_info
            .set_product_status(ProductStatus::Disabled)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_prods)))
}

// This only handles the refunding process AFTER finish
// For other refunding processes, see crate::orders
#[get("/refund_order?<order_id>")]
pub async fn refund_order(
    _auth: Auth<OrderRefundable>,
    order_id: OrderGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<Redirect, Flash<Redirect>> {
    let info = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    client
        .request(
            priv_key,
            RefundTrade::new(
                info.order_info.get_id(),
                "平台发起退货退款",
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
        .into_flash(uri!("/admin", admin_orders))?;
    Ok(Redirect::to(uri!("/admin", admin_orders)))
}

#[get("/finish_order?<order_id>")]
pub async fn finish_order(
    _auth: Auth<OrderFinishable>,
    order_id: OrderGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let info = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    conn.run(|c| {
        info.order_info
            .set_transaction_status(TransactionStatus::Finished)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", admin_orders)))
}

#[post("/cow_coupon?<coupon_id>", data = "<info>", rank = 1)]
pub async fn update_coupon(
    coupon_id: CouponGuard,
    _role: Role<Admin>,
    info: Form<Coupon>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let coupon = coupon_id.to_coupon(&conn).await.into_flash(uri!("/"))?;
    let id = coupon.get_id().to_string();
    conn.run(move |c| info.into_inner().update(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", update_coupon_page(id))))
}

#[post("/cow_coupon", data = "<info>", rank = 2)]
pub async fn create_coupon(
    _role: Role<Admin>,
    info: Form<Coupon>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| info.into_inner().create(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", coupons_page)))
}

#[get("/delete_coupon?<coupon_id>")]
pub async fn delete_coupon(
    coupon_id: CouponGuard,
    _role: Role<Admin>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let coupon = coupon_id.to_coupon(&conn).await.into_flash(uri!("/"))?;
    conn.run(move |c| coupon.delete(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/admin", coupons_page)))
}
