use crate::DbConn;
use rocket::{
    form::{self, FromFormField, ValueField},
    http::uri::fmt::{FromUriParam, Query},
};
use sails_db::{error::SailsDbError, tags::*};

#[derive(UriDisplayQuery)]
pub struct TagGuard(String);

impl<'v> FromFormField<'v> for TagGuard {
    #[inline]
    fn from_value(field: ValueField<'v>) -> form::Result<'v, Self> {
        Ok(Self(
            field.value.parse().map_err(form::error::Error::custom)?,
        ))
    }
}

impl<T: ToString> FromUriParam<Query, T> for TagGuard {
    type Target = TagGuard;

    fn from_uri_param(id: T) -> Self {
        Self(id.to_string())
    }
}

impl TagGuard {
    pub async fn to_tag(&self, db: &DbConn) -> Result<Tag, SailsDbError> {
        let tag_inner = self.0.clone();
        db.run(move |c| Tags::find_by_id(c, &tag_inner)).await
    }
}
