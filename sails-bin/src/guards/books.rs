use crate::DbConn;
use rocket::{
    form::{self, FromFormField, ValueField},
    http::uri::fmt::{FromUriParam, Query},
};
use sails_db::{
    categories::{Categories, Category},
    error::SailsDbError,
    products::*,
    users::*,
};

// TODO: we don't know why we are required to derive UriDisplayQuery instead of UriDisplayPath
#[derive(UriDisplayQuery)]
pub struct BookGuard(String);

impl<'v> FromFormField<'v> for BookGuard {
    #[inline]
    fn from_value(field: ValueField<'v>) -> form::Result<'v, Self> {
        Ok(BookGuard(
            field.value.parse().map_err(form::error::Error::custom)?,
        ))
    }
}

impl<T: ToString> FromUriParam<Query, T> for BookGuard {
    type Target = BookGuard;

    fn from_uri_param(id: T) -> BookGuard {
        BookGuard(id.to_string())
    }
}

impl BookGuard {
    pub async fn to_id(&self, db: &DbConn) -> Result<BookId, SailsDbError> {
        let book_id_inner = self.0.clone();
        db.run(move |c| -> Result<BookId, SailsDbError> {
            let book_id = ProductFinder::new(c, None).id(&book_id_inner).first()?;
            let seller_id = UserFinder::new(c, None)
                .id(book_id.get_info(c)?.get_seller_id())
                .first()?;
            let operator_id = UserFinder::new(c, None)
                .id(book_id.get_info(c)?.get_operator_id())
                .first()?;
            Ok(BookId {
                book_id,
                seller_id,
                operator_id,
            })
        })
        .await
    }

    pub async fn to_info(&self, db: &DbConn) -> Result<BookInfo<ProductInfo>, SailsDbError> {
        let book = self.to_id(db).await?;
        db.run(move |c| -> Result<BookInfo<_>, SailsDbError> {
            let book_info = book.book_id.get_info(c)?;
            let category = Categories::find_by_id(c, book_info.get_category_id()).ok();
            Ok(BookInfo {
                book_info,
                seller_info: book.seller_id.get_info(c)?,
                category,
            })
        })
        .await
    }

    pub async fn to_mut_info(
        &self,
        db: &DbConn,
    ) -> Result<BookInfo<MutableProductInfo>, SailsDbError> {
        let book = self.to_id(db).await?;
        db.run(move |c| -> Result<BookInfo<_>, SailsDbError> {
            let book_info = book.book_id.get_info(c)?.verify(c)?;
            let category = Categories::find_by_id(c, book_info.get_category_id()).ok();
            Ok(BookInfo {
                book_info,
                seller_info: book.seller_id.get_info(c)?,
                category,
            })
        })
        .await
    }
}

// This request guard explicitly requires a valid book ID
pub struct BookId {
    pub book_id: ProductId,
    pub seller_id: UserId,
    pub operator_id: UserId,
}

pub struct BookInfo<T> {
    pub book_info: T,
    pub seller_info: UserInfo,
    pub category: Option<Category>,
}
