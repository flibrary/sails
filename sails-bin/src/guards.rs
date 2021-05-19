use crate::DbConn;
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
};
use sails_db::{
    error::SailsDbError,
    products::{Product, ProductFinder},
    users::{User, Users},
};

// This request guard gets us an user if the user ID is specified and validated
pub struct UserGuard {
    pub user: User,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let uid = request
            .cookies()
            .get_private("uid")
            .map(|cookie| cookie.value().to_string());
        if let Some(uid) = uid {
            let uid_inner = uid.clone();
            db.run(move |c| -> Result<UserGuard, SailsDbError> {
                Ok(UserGuard {
                    user: Users::find_by_id(c, &uid_inner)?,
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

// This request guard explicitly requires a valid book ID
pub struct BookGuard {
    pub book: Product,
    pub seller: User,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BookGuard {
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
            db.run(move |c| -> Result<BookGuard, SailsDbError> {
                let book = ProductFinder::new(c, None)
                    .id(&book_id_inner)
                    .search()
                    // WARN: Ok() doesn't imply that there is at least one element
                    .and_then(|mut p| {
                        if !p.is_empty() {
                            Ok(p.remove(0))
                        } else {
                            Err(SailsDbError::ProductNotFound)
                        }
                    })?;
                let seller = Users::find_by_id(c, book.get_seller_id())?;
                Ok(BookGuard { book, seller })
            })
            .await
            .ok()
            .or_forward(())
        } else {
            Outcome::Forward(())
        }
    }
}

// This guard matches only if the user is authorized. It implies also that Book is present and User is present
pub struct Authorized;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Authorized {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserGuard>().await);
        let book = try_outcome!(request.guard::<BookGuard>().await);
        if book.seller.get_id() == user.user.get_id() {
            Outcome::Success(Authorized)
        } else {
            Outcome::Forward(())
        }
    }
}
