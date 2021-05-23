use crate::DbConn;
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
};
use sails_db::{error::SailsDbError, products::*, users::*};

// This request guard gets us an user if the user ID is specified and validated
pub struct UserIdGuard {
    pub id: UserId,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserIdGuard {
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
            db.run(move |c| -> Result<UserIdGuard, SailsDbError> {
                Ok(UserIdGuard {
                    id: UserFinder::new(c, None).id(&uid_inner).first()?,
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
pub struct BookIdGuard {
    pub book_id: ProductId,
    pub seller_id: UserId,
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
                Ok(BookIdGuard { book_id, seller_id })
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
        let user = try_outcome!(request.guard::<UserIdGuard>().await);
        let book = try_outcome!(request.guard::<BookIdGuard>().await);
        if book.seller_id.get_id() == user.id.get_id() {
            Outcome::Success(Authorized)
        } else {
            Outcome::Forward(())
        }
    }
}

pub struct UserInfoGuard {
    pub info: UserInfo,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserInfoGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<UserInfoGuard, SailsDbError> {
            Ok(UserInfoGuard {
                info: user.id.get_info(c)?,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}

pub struct BookInfoGuard {
    pub book_info: ProductInfo,
    pub seller_info: UserInfo,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for BookInfoGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let book = try_outcome!(request.guard::<BookIdGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<BookInfoGuard, SailsDbError> {
            Ok(BookInfoGuard {
                book_info: book.book_id.get_info(c)?,
                seller_info: book.seller_id.get_info(c)?,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}

pub struct ReceiverIdGuard {
    pub id: UserId,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ReceiverIdGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let recv_id = request
            .query_value::<String>("receiver_id")
            .and_then(|x| x.ok());
        if let Some(uid) = recv_id {
            let uid_inner = uid.clone();
            db.run(move |c| -> Result<ReceiverIdGuard, SailsDbError> {
                Ok(ReceiverIdGuard {
                    id: UserFinder::new(c, None).id(&uid_inner).first()?,
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

pub struct ReceiverInfoGuard {
    pub info: UserInfo,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ReceiverInfoGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let receiver = try_outcome!(request.guard::<ReceiverIdGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<ReceiverInfoGuard, SailsDbError> {
            Ok(ReceiverInfoGuard {
                info: receiver.id.get_info(c)?,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}
