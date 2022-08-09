use crate::DbConn;
use rocket::{
    form::{self, FromFormField, ValueField},
    http::uri::fmt::{FromUriParam, Query},
};
use sails_db::{coupons::*, error::SailsDbError};

#[derive(UriDisplayQuery)]
pub struct CouponGuard(String);

impl<'v> FromFormField<'v> for CouponGuard {
    #[inline]
    fn from_value(field: ValueField<'v>) -> form::Result<'v, Self> {
        Ok(Self(
            field.value.parse().map_err(form::error::Error::custom)?,
        ))
    }
}

impl<T: ToString> FromUriParam<Query, T> for CouponGuard {
    type Target = CouponGuard;

    fn from_uri_param(id: T) -> Self {
        Self(id.to_string())
    }
}

impl CouponGuard {
    pub async fn to_coupon(&self, db: &DbConn) -> Result<Coupon, SailsDbError> {
        let coupon_inner = self.0.clone();
        db.run(move |c| CouponFinder::new(c, None).id(&coupon_inner).first())
            .await
    }
}
