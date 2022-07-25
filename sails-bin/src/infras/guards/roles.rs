use super::users::*;
use rocket::{
    outcome::{try_outcome, Outcome},
    request::FromRequest,
};
use sails_db::enums::UserStatus;
use std::marker::PhantomData;

pub struct Root;
pub struct Admin;
pub struct StoreKeeper;
pub struct CustomerService;
pub struct Normal;
pub struct Disabled;

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
impl<'r> FromRequest<'r> for Role<Admin> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        if user.info.get_user_status().contains(UserStatus::ADMIN) {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<StoreKeeper> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        if user
            .info
            .get_user_status()
            .contains(UserStatus::STORE_KEEPER)
        {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<CustomerService> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        if user
            .info
            .get_user_status()
            .contains(UserStatus::CUSTOMER_SERVICE)
        {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<Normal> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        if user.info.get_user_status().contains(UserStatus::NORMAL) {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Role<Disabled> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);
        // Disabled user has no permission, we CANNOT use contains otherwise everyone is disabled!
        if user.info.get_user_status() == UserStatus::DISABLED {
            Outcome::Success(Role { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}
