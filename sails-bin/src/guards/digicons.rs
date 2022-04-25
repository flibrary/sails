use crate::DbConn;
use rocket::{
    form::{self, FromFormField, ValueField},
    http::uri::fmt::{FromUriParam, Query},
};
use sails_db::{digicons::*, error::SailsDbError};

#[derive(UriDisplayQuery)]
pub struct DigiconGuard(String);

impl<'v> FromFormField<'v> for DigiconGuard {
    #[inline]
    fn from_value(field: ValueField<'v>) -> form::Result<'v, Self> {
        Ok(Self(
            field.value.parse().map_err(form::error::Error::custom)?,
        ))
    }
}

impl<T: ToString> FromUriParam<Query, T> for DigiconGuard {
    type Target = DigiconGuard;

    fn from_uri_param(id: T) -> Self {
        Self(id.to_string())
    }
}

impl DigiconGuard {
    pub async fn to_digicon(&self, db: &DbConn) -> Result<Digicon, SailsDbError> {
        let digicon_inner = self.0.clone();
        db.run(move |c| Digicons::find_by_id(c, &digicon_inner))
            .await
    }
}
