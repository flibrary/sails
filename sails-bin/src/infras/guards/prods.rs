use crate::DbConn;
use rocket::{
    form::{self, FromFormField, ValueField},
    http::uri::fmt::{FromUriParam, Query},
};
use sails_db::{
    categories::{Categories, Category},
    error::SailsDbError,
    products::*,
    tags::*,
    users::*,
};

// TODO: we don't know why we are required to derive UriDisplayQuery instead of UriDisplayPath
#[derive(UriDisplayQuery)]
pub struct ProdGuard(String);

impl<'v> FromFormField<'v> for ProdGuard {
    #[inline]
    fn from_value(field: ValueField<'v>) -> form::Result<'v, Self> {
        Ok(ProdGuard(
            field.value.parse().map_err(form::error::Error::custom)?,
        ))
    }
}

impl<T: ToString> FromUriParam<Query, T> for ProdGuard {
    type Target = ProdGuard;

    fn from_uri_param(id: T) -> ProdGuard {
        ProdGuard(id.to_string())
    }
}

impl ProdGuard {
    pub async fn to_id(&self, db: &DbConn) -> Result<ProdId, SailsDbError> {
        let prod_id_inner = self.0.clone();
        db.run(move |c| -> Result<ProdId, SailsDbError> {
            let prod_id = ProductFinder::new(c, None).id(&prod_id_inner).first()?;
            let seller_id = UserFinder::new(c, None)
                .id(prod_id.get_info(c)?.get_seller_id())
                .first()?;
            Ok(ProdId { prod_id, seller_id })
        })
        .await
    }

    pub async fn to_info(&self, db: &DbConn) -> Result<ProdInfo<ProductInfo>, SailsDbError> {
        let prod = self.to_id(db).await?;
        db.run(move |c| -> Result<ProdInfo<_>, SailsDbError> {
            let prod_info = prod.prod_id.get_info(c)?;
            let category = Categories::find_by_id(c, prod_info.get_category_id()).ok();
            let tags = TagMappingFinder::new(c, None)
                .product(&prod.prod_id)
                .search_tag()?;
            Ok(ProdInfo {
                prod_info,
                seller_info: prod.seller_id.get_info(c)?,
                category,
                tags,
            })
        })
        .await
    }
}

// This request guard explicitly requires a valid prod ID
pub struct ProdId {
    pub prod_id: ProductId,
    pub seller_id: UserId,
}

pub struct ProdInfo<T> {
    pub prod_info: T,
    pub seller_info: UserInfo,
    pub category: Option<Category>,
    pub tags: Vec<Tag>,
}
