use super::{books::*, orders::*, users::*};
use rocket::{
    outcome::{try_outcome, Outcome},
    request::FromRequest,
};
use std::marker::PhantomData;

pub struct Root;
pub struct Admin;
pub struct BookAuthorized;
pub struct Buyer;
pub struct Seller;
pub struct Role<T> {
    plhdr: PhantomData<T>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<Root> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        match request
            .cookies()
            .get_private("root_challenge")
            .map(|cookie| cookie.value().to_string())
        {
            Some(s) if s == "ROOT" => Outcome::Success(Role { plhdr: PhantomData }),
            _ => Outcome::Forward(()),
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<Seller> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        let order = try_outcome!(request.guard::<OrderInfoGuard>().await);
        if order.book_info.get_seller_id() == user.info.get_id() {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<Buyer> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        let order = try_outcome!(request.guard::<OrderInfoGuard>().await);
        if order.order_info.get_buyer() == user.info.get_id() {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<BookAuthorized> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        let book = try_outcome!(request.guard::<BookIdGuard>().await);
        if (book.seller_id.get_id() == user.info.get_id()) || user.info.is_admin() {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<Admin> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        if user.info.is_admin() {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}
