use crate::DbConn;
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
};
use sails_db::{
    categories::{Categories, Category},
    error::SailsDbError,
    products::*,
    users::*,
};

// This request guard explicitly requires a valid book ID
pub struct BookIdGuard {
    pub book_id: ProductId,
    pub seller_id: UserId,
    pub operator_id: UserId,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BookIdGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let book_id = request
            .query_value::<String>("book_id")
            .and_then(|x| x.ok());
        if let Some(book_id) = book_id {
            let book_id_inner = book_id.clone();
            db.run(move |c| -> Result<BookIdGuard, SailsDbError> {
                let book_id = ProductFinder::new(c, None).id(&book_id_inner).first()?;
                let seller_id = UserFinder::new(c, None)
                    .id(book_id.get_info(c)?.get_seller_id())
                    .first()?;
                let operator_id = UserFinder::new(c, None)
                    .id(book_id.get_info(c)?.get_operator_id())
                    .first()?;
                Ok(BookIdGuard {
                    book_id,
                    seller_id,
                    operator_id,
                })
            })
            .await
            .ok()
            .or_forward(())
        } else {
            Outcome::Forward(())
        }
    }
}

pub struct BookInfoGuard<T> {
    pub book_info: T,
    pub seller_info: UserInfo,
    pub category: Option<Category>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BookInfoGuard<MutableProductInfo> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let book = try_outcome!(request.guard::<BookIdGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<BookInfoGuard<_>, SailsDbError> {
            let book_info = book.book_id.get_info(c)?.verify(c)?;
            let category = Categories::find_by_id(c, book_info.get_category_id()).ok();
            Ok(BookInfoGuard {
                book_info,
                seller_info: book.seller_id.get_info(c)?,
                category,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BookInfoGuard<ProductInfo> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let book = try_outcome!(request.guard::<BookIdGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<BookInfoGuard<_>, SailsDbError> {
            let book_info = book.book_id.get_info(c)?;
            let category = Categories::find_by_id(c, book_info.get_category_id()).ok();
            Ok(BookInfoGuard {
                book_info,
                seller_info: book.seller_id.get_info(c)?,
                category,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}
