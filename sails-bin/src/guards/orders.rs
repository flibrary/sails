use crate::DbConn;
use rocket::{
    form::{self, FromFormField, ValueField},
    http::uri::fmt::{FromUriParam, Query},
};
use sails_db::{
    error::SailsDbError,
    products::*,
    transactions::*,
    users::{UserFinder, UserInfo},
};

#[derive(UriDisplayQuery)]
pub struct OrderGuard(String);

impl<'v> FromFormField<'v> for OrderGuard {
    #[inline]
    fn from_value(field: ValueField<'v>) -> form::Result<'v, Self> {
        Ok(OrderGuard(
            field.value.parse().map_err(form::error::Error::custom)?,
        ))
    }
}

impl<T: ToString> FromUriParam<Query, T> for OrderGuard {
    type Target = OrderGuard;

    fn from_uri_param(id: T) -> OrderGuard {
        OrderGuard(id.to_string())
    }
}

impl OrderGuard {
    pub async fn to_id(&self, db: &DbConn) -> Result<OrderId, SailsDbError> {
        let order_id_inner = self.0.clone();
        db.run(move |c| -> Result<OrderId, SailsDbError> {
            Ok(OrderId {
                id: TransactionFinder::new(c, None)
                    .id(&order_id_inner)
                    .first()?,
            })
        })
        .await
    }

    pub async fn to_info(&self, db: &DbConn) -> Result<OrderInfo, SailsDbError> {
        let order = self.to_id(db).await?;
        db.run(move |c| -> Result<OrderInfo, SailsDbError> {
            let order_info = order.id.get_info(c)?;
            let book_info = ProductFinder::new(c, None)
                .id(order_info.get_product())
                .first_info()?;
            let seller_info = UserFinder::new(c, None)
                .id(order_info.get_seller())
                .first_info()?;
            let buyer_info = UserFinder::new(c, None)
                .id(order_info.get_buyer())
                .first_info()?;

            Ok(OrderInfo {
                order_info,
                book_info,
                seller_info,
                buyer_info,
            })
        })
        .await
    }
}

#[derive(Clone)]
pub struct OrderInfo {
    pub order_info: TransactionInfo,
    pub book_info: ProductInfo,
    pub seller_info: UserInfo,
    pub buyer_info: UserInfo,
}

// This request guard explicitly requires a valid transaction ID
pub struct OrderId {
    pub id: TransactionId,
}
