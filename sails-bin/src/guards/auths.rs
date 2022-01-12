use super::{books::*, orders::*, users::*};
use crate::DbConn;
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
};
use sails_db::enums::UserStatus;
use std::marker::PhantomData;

// For books
pub struct BookReadable;
pub struct BookWritable;
pub struct BookRemovable;
pub struct BookAdmin;

// For users
pub struct UserReadable;
pub struct UserWritable;

// For orders
pub struct OrderReadable;
pub struct OrderProgressable;
pub struct OrderFinishable;
pub struct OrderRefundable;

pub struct Auth<T> {
    plhdr: PhantomData<T>,
}

// Books
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<BookAdmin> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        if user.info.get_user_status().contains(UserStatus::PROD_ADMIN) {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<BookReadable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let book = try_outcome!(request
            .query_value::<BookGuard>("book_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));
        let book = try_outcome!(book.to_id(&db).await.ok().or_forward(()));

        if match (
            book.operator_id.get_id() == user.info.get_id(),
            book.seller_id.get_id() == user.info.get_id(),
        ) {
            // seller
            (true, true)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::PROD_SELF_READABLE) =>
            {
                true
            }
            // delegator
            (true, false)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::PROD_DELG_READABLE) =>
            {
                true
            }
            _ if user
                .info
                .get_user_status()
                .contains(UserStatus::PROD_OTHERS_READABLE) =>
            {
                true
            }
            _ => false,
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<BookWritable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let book = try_outcome!(request
            .query_value::<BookGuard>("book_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));
        let book = try_outcome!(book.to_id(&db).await.ok().or_forward(()));

        if match (
            book.operator_id.get_id() == user.info.get_id(),
            book.seller_id.get_id() == user.info.get_id(),
        ) {
            // seller
            (true, true)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::PROD_SELF_WRITABLE) =>
            {
                true
            }
            // delegator
            (true, false)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::PROD_DELG_WRITABLE) =>
            {
                true
            }
            _ if user
                .info
                .get_user_status()
                .contains(UserStatus::PROD_OTHERS_WRITABLE) =>
            {
                true
            }
            _ => false,
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<BookRemovable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let book = try_outcome!(request
            .query_value::<BookGuard>("book_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));
        let book = try_outcome!(book.to_id(&db).await.ok().or_forward(()));

        if match (
            book.operator_id.get_id() == user.info.get_id(),
            book.seller_id.get_id() == user.info.get_id(),
        ) {
            // seller
            (true, true)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::PROD_SELF_REMOVABLE) =>
            {
                true
            }
            // delegator
            (true, false)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::PROD_DELG_REMOVABLE) =>
            {
                true
            }
            _ if user
                .info
                .get_user_status()
                .contains(UserStatus::PROD_OTHERS_REMOVABLE) =>
            {
                true
            }
            _ => false,
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

// Users
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<UserReadable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let user_param = try_outcome!(request
            .query_value::<UserGuard>("user_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));

        if match user_param.to_id_param(&db).await {
            Ok(_) => user
                .info
                .get_user_status()
                .contains(UserStatus::USER_OTHERS_READABLE),
            Err(_) => user
                .info
                .get_user_status()
                .contains(UserStatus::USER_SELF_READABLE),
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<UserWritable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let user_param = try_outcome!(request
            .query_value::<UserGuard>("user_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));

        if match user_param.to_id_param(&db).await {
            Ok(_) => user
                .info
                .get_user_status()
                .contains(UserStatus::USER_OTHERS_WRITABLE),
            Err(_) => user
                .info
                .get_user_status()
                .contains(UserStatus::USER_SELF_WRITABLE),
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

// Orders
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<OrderReadable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if match (
            user.info.get_id() == order.order_info.get_buyer(),
            user.info.get_id() == order.order_info.get_seller(),
        ) {
            (true, false)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_BUYER_READABLE) =>
            {
                true
            }
            (false, true)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_SELLER_READABLE) =>
            {
                true
            }
            _ if user
                .info
                .get_user_status()
                .contains(UserStatus::TX_OTHERS_READABLE) =>
            {
                true
            }
            _ => false,
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<OrderProgressable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if match (
            user.info.get_id() == order.order_info.get_buyer(),
            user.info.get_id() == order.order_info.get_seller(),
        ) {
            (true, false)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_BUYER_PROGRESSABLE) =>
            {
                true
            }
            (false, true)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_SELLER_PROGRESSABLE) =>
            {
                true
            }
            _ if user
                .info
                .get_user_status()
                .contains(UserStatus::TX_OTHERS_PROGRESSABLE) =>
            {
                true
            }
            _ => false,
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<OrderFinishable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if match (
            user.info.get_id() == order.order_info.get_buyer(),
            user.info.get_id() == order.order_info.get_seller(),
        ) {
            (true, false)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_BUYER_FINISHABLE) =>
            {
                true
            }
            (false, true)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_SELLER_FINISHABLE) =>
            {
                true
            }
            _ if user
                .info
                .get_user_status()
                .contains(UserStatus::TX_OTHERS_FINISHABLE) =>
            {
                true
            }
            _ => false,
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<OrderRefundable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .map(|x| x.ok())
            .flatten()
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if match (
            user.info.get_id() == order.order_info.get_buyer(),
            user.info.get_id() == order.order_info.get_seller(),
        ) {
            (true, false)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_BUYER_REFUNDABLE) =>
            {
                true
            }
            (false, true)
                if user
                    .info
                    .get_user_status()
                    .contains(UserStatus::TX_SELLER_REFUNDABLE) =>
            {
                true
            }
            _ if user
                .info
                .get_user_status()
                .contains(UserStatus::TX_OTHERS_REFUNDABLE) =>
            {
                true
            }
            _ => false,
        } {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}
