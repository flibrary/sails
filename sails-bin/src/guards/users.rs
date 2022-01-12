use crate::{aead::AeadKey, DbConn};
use chrono::{DateTime, NaiveDateTime, Utc};
use rocket::{
    form::{self, FromFormField, ValueField},
    http::uri::fmt::{FromUriParam, Query},
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
    State,
};
use sails_db::{error::SailsDbError, users::*};
use std::marker::PhantomData;

// User ID sepcified as AEAD encrypted ciphertext
pub struct Aead;

// User ID specified in param
pub struct Param;

// User ID stored in private cookie
pub struct Cookie;

#[derive(UriDisplayQuery)]
pub struct UserGuard(String);

impl<'v> FromFormField<'v> for UserGuard {
    #[inline]
    fn from_value(field: ValueField<'v>) -> form::Result<'v, Self> {
        Ok(UserGuard(
            field.value.parse().map_err(form::error::Error::custom)?,
        ))
    }
}

impl<T: ToString> FromUriParam<Query, T> for UserGuard {
    type Target = UserGuard;

    fn from_uri_param(id: T) -> UserGuard {
        UserGuard(id.to_string())
    }
}

impl UserGuard {
    pub async fn to_id_param(&self, db: &DbConn) -> Result<UserIdGuard<Param>, SailsDbError> {
        let uid_inner = self.0.clone();
        db.run(move |c| -> Result<UserIdGuard<Param>, SailsDbError> {
            Ok(UserIdGuard {
                id: UserFinder::new(c, None).id(&uid_inner).first()?,
                plhdr: PhantomData,
            })
        })
        .await
    }

    pub async fn to_id_aead(
        &self,
        db: &DbConn,
        exp: i64,
        aead: &State<AeadKey>,
    ) -> Result<UserIdGuard<Aead>, SailsDbError> {
        if Utc::now() <= DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(exp, 0), Utc) {
            let decode_fn = || -> Result<String, anyhow::Error> {
                let decoded = base64::decode_config(&self.0, base64::URL_SAFE)?;
                Ok(String::from_utf8(
                    aead.decrypt(&decoded, &crate::user::timestamp_to_nonce(exp))
                        .map_err(|_| anyhow::anyhow!("mailaddress decryption failed"))?,
                )?)
            };

            let uid = decode_fn();

            db.run(move |c| -> Result<UserIdGuard<Aead>, SailsDbError> {
                Ok(UserIdGuard {
                    id: UserFinder::new(c, None).id(&uid?).first()?,
                    plhdr: PhantomData,
                })
            })
            .await
        } else {
            Err(SailsDbError::IllegalQuery)
        }
    }

    pub async fn to_info_param(&self, db: &DbConn) -> Result<UserInfoGuard<Param>, SailsDbError> {
        let id = self.to_id_param(db).await?;
        db.run(move |c| -> Result<UserInfoGuard<Param>, SailsDbError> {
            Ok(UserInfoGuard {
                info: id.id.get_info(c)?,
                plhdr: PhantomData,
            })
        })
        .await
    }

    pub async fn to_info_aead(
        &self,
        db: &DbConn,
        exp: i64,
        aead: &State<AeadKey>,
    ) -> Result<UserInfoGuard<Aead>, SailsDbError> {
        let id = self.to_id_aead(db, exp, aead).await?;
        db.run(move |c| -> Result<UserInfoGuard<Aead>, SailsDbError> {
            Ok(UserInfoGuard {
                info: id.id.get_info(c)?,
                plhdr: PhantomData,
            })
        })
        .await
    }
}

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
