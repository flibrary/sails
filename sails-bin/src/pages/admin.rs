use crate::{
    infras::{guards::*, i18n::I18n},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{
    coupons::*,
    enums::{ProductStatus, TransactionStatus},
    error::SailsDbError,
    products::{ProductFinder, ProductInfo},
    tags::*,
    transactions::*,
    users::{UserFinder, UserStats},
    Cmp,
};

#[derive(Template)]
#[template(path = "admin/metrics.html")]
pub struct AdminMetricsPage {
    i18n: I18n,
    pub order: TxStats,
    pub user: UserStats,
}

// To prevent deadlock, redirect all errors back to index as this is the default route for `/admin`
#[get("/metrics")]
pub async fn admin_metrics(
    i18n: I18n,
    _guard: Role<Admin>,
    conn: DbConn,
) -> Result<AdminMetricsPage, Flash<Redirect>> {
    Ok(AdminMetricsPage {
        i18n,
        order: conn
            .run(|c| TransactionFinder::stats(c, None))
            .await
            .into_flash(uri!("/"))?,
        user: conn
            .run(|c| UserFinder::stats(c))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[derive(Template)]
#[template(path = "admin/orders.html")]
pub struct AdminOrdersPage {
    i18n: I18n,
    paid_tx: Vec<(ProductInfo, TransactionInfo)>,
    placed_tx: Vec<(ProductInfo, TransactionInfo)>,
    refunded_tx: Vec<(ProductInfo, TransactionInfo)>,
    finished_tx: Vec<(ProductInfo, TransactionInfo)>,
}

// CustomerService or above can READ all orders
#[get("/orders")]
pub async fn admin_orders(
    i18n: I18n,
    _guard: Role<CustomerService>,
    conn: DbConn,
) -> Result<AdminOrdersPage, Flash<Redirect>> {
    let paid_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Paid, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let refunded_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Refunded, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let placed_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Placed, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    let finished_tx = conn
        .run(
            |c| -> Result<Vec<(ProductInfo, TransactionInfo)>, SailsDbError> {
                Ok(TransactionFinder::new(c, None)
                    .status(TransactionStatus::Finished, Cmp::Equal)
                    .search_info()?
                    .into_iter()
                    .map(|x| {
                        (
                            ProductFinder::new(c, None)
                                .id(x.get_product())
                                .first_info()
                                .unwrap(),
                            x,
                        )
                    })
                    .collect())
            },
        )
        .await
        .into_flash(uri!("/"))?;

    Ok(AdminOrdersPage {
        i18n,
        paid_tx,
        placed_tx,
        refunded_tx,
        finished_tx,
    })
}

#[derive(Template)]
#[template(path = "admin/tag.html")]
pub struct AdminTagPage {
    i18n: I18n,
    tag: Tag,
    tagged: Vec<ProductInfo>,
    untagged: Vec<ProductInfo>,
}

#[get("/tag?<id>")]
pub async fn admin_tag(
    i18n: I18n,
    _guard: Auth<TagWritable>,
    id: TagGuard,
    conn: DbConn,
) -> Result<AdminTagPage, Flash<Redirect>> {
    let id = id.to_tag(&conn).await.into_flash(uri!("/"))?;
    let tag = id.clone();
    let (tagged, untagged) = conn
        .run(
            move |c| -> Result<(Vec<ProductInfo>, Vec<ProductInfo>), SailsDbError> {
                let tagged = ProductFinder::list_info(c).map(|x| {
                    x.into_iter()
                        .filter(|p| {
                            TagMappingFinder::has_mapping(c, &tag, &p.to_id()).unwrap_or(false)
                        })
                        .collect()
                })?;
                let untagged = ProductFinder::list_info(c).map(|x| {
                    x.into_iter()
                        .filter(|p| {
                            !TagMappingFinder::has_mapping(c, &tag, &p.to_id()).unwrap_or(false)
                        })
                        .collect()
                })?;
                Ok((tagged, untagged))
            },
        )
        .await
        .into_flash(uri!("/"))?;

    Ok(AdminTagPage {
        i18n,
        tag: id,
        tagged,
        untagged,
    })
}

#[derive(Template)]
#[template(path = "admin/tags.html")]
pub struct AdminTagsPage {
    i18n: I18n,
    tags: Vec<Tag>,
}

#[get("/tags")]
pub async fn admin_tags(
    i18n: I18n,
    _guard: Auth<TagWritable>,
    conn: DbConn,
) -> Result<AdminTagsPage, Flash<Redirect>> {
    Ok(AdminTagsPage {
        i18n,
        tags: conn
            .run(|c| Tags::list_all(c))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[derive(Template)]
#[template(path = "admin/prods.html")]
pub struct AdminProdsPage {
    i18n: I18n,
    verified_prods: Vec<ProductInfo>,
    disabled_prods: Vec<ProductInfo>,
}

// If the user has already been verified, show him the root dashboard
#[get("/prods")]
pub async fn admin_prods(
    i18n: I18n,
    _guard: Auth<ProdAdmin>,
    conn: DbConn,
) -> Result<AdminProdsPage, Flash<Redirect>> {
    let disabled_prods = conn
        .run(|c| {
            ProductFinder::new(c, None)
                .status(ProductStatus::Disabled, Cmp::Equal)
                .search_info()
        })
        .await
        .into_flash(uri!("/"))?;

    let verified_prods = conn
        .run(|c| {
            ProductFinder::new(c, None)
                .status(ProductStatus::Verified, Cmp::Equal)
                .search_info()
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(AdminProdsPage {
        i18n,
        disabled_prods,
        verified_prods,
    })
}

#[derive(Template)]
#[template(path = "admin/order_info_admin.html")]
pub struct AdminOrderInfoPage {
    i18n: I18n,
    prod: ProductInfo,
    order: TransactionInfo,
}

#[get("/order_info?<order_id>")]
pub async fn order_info(
    i18n: I18n,
    // CustomerService imply OrderOthersReadable, which is what this admin page is for.
    _auth: Role<CustomerService>,
    order_id: OrderGuard,
    conn: DbConn,
) -> Result<AdminOrderInfoPage, Flash<Redirect>> {
    let order = order_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(AdminOrderInfoPage {
        i18n,
        prod: order.prod_info,
        order: order.order_info,
    })
}

#[derive(Template)]
#[template(path = "admin/coupons/create_coupon.html")]
pub struct AdminCreateCouponPage {
    i18n: I18n,
}

#[get("/create_coupon")]
pub async fn create_coupon_page(
    i18n: I18n,
    _role: Role<Admin>,
) -> Result<AdminCreateCouponPage, Flash<Redirect>> {
    Ok(AdminCreateCouponPage { i18n })
}

#[derive(Template)]
#[template(path = "admin/coupons/update_coupon.html")]
pub struct AdminUpdateCouponPage {
    i18n: I18n,
    coupon: Coupon,
}

#[get("/create_coupon?<coupon_id>")]
pub async fn update_coupon_page(
    i18n: I18n,
    coupon_id: CouponGuard,
    _role: Role<Admin>,
    conn: DbConn,
) -> Result<AdminUpdateCouponPage, Flash<Redirect>> {
    let coupon = coupon_id.to_coupon(&conn).await.into_flash(uri!("/"))?;
    Ok(AdminUpdateCouponPage { i18n, coupon })
}

#[derive(Template)]
#[template(path = "admin/coupons/coupons.html")]
pub struct AdminCouponsPage {
    i18n: I18n,
    coupons: Vec<Coupon>,
}

#[get("/coupons")]
pub async fn coupons_page(
    i18n: I18n,
    _role: Role<Admin>,
    conn: DbConn,
) -> Result<AdminCouponsPage, Flash<Redirect>> {
    let coupons = conn
        .run(move |c| CouponFinder::list(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(AdminCouponsPage { i18n, coupons })
}

#[get("/")]
pub async fn admin(_guard: Auth<ProdAdmin>) -> Redirect {
    Redirect::to(uri!("/admin", admin_metrics))
}
