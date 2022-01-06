use std::marker::PhantomData;

use crate::{aead::AeadKey, DbConn};
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
    State,
};
use sails_db::{error::SailsDbError, users::*};

// User ID indicated in the URL param
pub struct Param;

// User ID stored in private cookie
pub struct Cookie;

// User ID indicated in the URL param but encrypted with our secret key in AEAD
pub struct Aead;

// This request guard gets us an user if the user ID is specified and validated
pub struct UserIdGuard<T> {
    pub id: UserId,
    plhdr: PhantomData<T>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserIdGuard<Cookie> {
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
            db.run(move |c| -> Result<UserIdGuard<_>, SailsDbError> {
                Ok(UserIdGuard {
                    // Disabled user will be treated as if he is not logged in
                    id: UserFinder::new(c, None).id(&uid_inner).allowed().first()?,
                    plhdr: PhantomData,
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserIdGuard<Param> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let user_id = request
            .query_value::<String>("user_id")
            .and_then(|x| x.ok());
        if let Some(uid) = user_id {
            let uid_inner = uid.clone();
            db.run(move |c| -> Result<UserIdGuard<Param>, SailsDbError> {
                Ok(UserIdGuard {
                    id: UserFinder::new(c, None).id(&uid_inner).first()?,
                    plhdr: PhantomData,
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserIdGuard<Aead> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let aead = try_outcome!(request.guard::<&State<AeadKey>>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);

        let key = request
            .query_value::<String>("enc_user_id")
            .and_then(|x| x.ok());
        if let Some(key) = key {
            let decode_fn = || -> Result<String, anyhow::Error> {
                let decoded = base64::decode_config(&key, base64::URL_SAFE)?;
                Ok(String::from_utf8(aead.decrypt(&decoded).map_err(
                    |_| anyhow::anyhow!("mailaddress decryption failed"),
                )?)?)
            };

            let uid = decode_fn();

            db.run(move |c| -> Result<UserIdGuard<Aead>, anyhow::Error> {
                Ok(UserIdGuard {
                    id: UserFinder::new(c, None).id(&uid?).first()?,
                    plhdr: PhantomData,
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

pub struct UserInfoGuard<T> {
    pub info: UserInfo,
    plhdr: PhantomData<T>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserInfoGuard<Cookie> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<UserInfoGuard<Cookie>, SailsDbError> {
            Ok(UserInfoGuard {
                info: user.id.get_info(c)?,
                plhdr: PhantomData,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserInfoGuard<Param> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let id = try_outcome!(request.guard::<UserIdGuard<Param>>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<UserInfoGuard<Param>, SailsDbError> {
            Ok(UserInfoGuard {
                info: id.id.get_info(c)?,
                plhdr: PhantomData,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserInfoGuard<Aead> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let aead = try_outcome!(request.guard::<&State<AeadKey>>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);

        let key = request
            .query_value::<String>("enc_user_id")
            .and_then(|x| x.ok());
        if let Some(key) = key {
            let decode_fn = || -> Result<String, anyhow::Error> {
                let decoded = base64::decode_config(&key, base64::URL_SAFE)?;
                Ok(String::from_utf8(aead.decrypt(&decoded).map_err(
                    |_| anyhow::anyhow!("mailaddress decryption failed"),
                )?)?)
            };

            let uid = decode_fn();

            db.run(move |c| -> Result<UserInfoGuard<Aead>, anyhow::Error> {
                Ok(UserInfoGuard {
                    info: UserFinder::new(c, None).id(&uid?).first_info()?,
                    plhdr: PhantomData,
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
