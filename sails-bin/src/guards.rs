use crate::{aead::AeadKey, DbConn};
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
    State,
};
use sails_db::{
    categories::{Categories, Category},
    error::SailsDbError,
    products::*,
    transactions::*,
    users::*,
};

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
                    // Disabled user will be treated as if he is not logged in
                    id: UserFinder::new(c, None).id(&uid_inner).allowed().first()?,
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

pub struct RootGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for RootGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        match request
            .cookies()
            .get_private("root_challenge")
            .map(|cookie| cookie.value().to_string())
        {
            Some(s) if s == "ROOT" => Outcome::Success(RootGuard),
            _ => Outcome::Forward(()),
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

// This guard matches only if the user is the seller of the order;
pub struct OrderSeller;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OrderSeller {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard>().await);
        let order = try_outcome!(request.guard::<OrderInfoGuard>().await);
        if order.book_info.get_seller_id() == user.info.get_id() {
            Outcome::Success(OrderSeller)
        } else {
            Outcome::Forward(())
        }
    }
}

// This guard matches only if the user is the buyer of the order;
pub struct OrderBuyer;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OrderBuyer {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard>().await);
        let order = try_outcome!(request.guard::<OrderInfoGuard>().await);
        if order.order_info.get_buyer() == user.info.get_id() {
            Outcome::Success(OrderBuyer)
        } else {
            Outcome::Forward(())
        }
    }
}

pub struct OrderInfoGuard {
    pub order_info: TransactionInfo,
    pub book_info: ProductInfo,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OrderInfoGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let order = try_outcome!(request.guard::<OrderIdGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<OrderInfoGuard, SailsDbError> {
            let order_info = order.id.get_info(c)?;
            let book_info = ProductFinder::new(c, None)
                .id(order_info.get_product())
                .first_info()?;
            Ok(OrderInfoGuard {
                order_info,
                book_info,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}

// This request guard explicitly requires a valid transaction ID
pub struct OrderIdGuard {
    pub id: TransactionId,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OrderIdGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let order_id = request
            .query_value::<String>("order_id")
            .and_then(|x| x.ok());
        if let Some(order_id) = order_id {
            let order_id_inner = order_id.clone();
            db.run(move |c| -> Result<OrderIdGuard, SailsDbError> {
                Ok(OrderIdGuard {
                    id: TransactionFinder::new(c, None)
                        .id(&order_id_inner)
                        .first()?,
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

// This guard matches only if the user is authorized. It implies also that Book is present and User is present
pub struct Authorized;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Authorized {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard>().await);
        let book = try_outcome!(request.guard::<BookIdGuard>().await);
        if (book.seller_id.get_id() == user.info.get_id()) || user.info.is_admin() {
            Outcome::Success(Authorized)
        } else {
            Outcome::Forward(())
        }
    }
}

pub struct AdminGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard>().await);
        if user.info.is_admin() {
            Outcome::Success(AdminGuard)
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

pub struct MutableBookInfoGuard {
    pub info: MutableProductInfo,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for MutableBookInfoGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let book = try_outcome!(request.guard::<BookInfoGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(|c| -> Result<Self, SailsDbError> {
            Ok(Self {
                info: book.book_info.verify(c)?,
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
    pub category: Option<Category>,
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

pub struct UserIdParamGuard {
    pub id: UserId,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserIdParamGuard {
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
            db.run(move |c| -> Result<UserIdParamGuard, SailsDbError> {
                Ok(UserIdParamGuard {
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

pub struct UserInfoParamGuard {
    pub info: UserInfo,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserInfoParamGuard {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let id = try_outcome!(request.guard::<UserIdParamGuard>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);
        db.run(move |c| -> Result<UserInfoParamGuard, SailsDbError> {
            Ok(UserInfoParamGuard {
                info: id.id.get_info(c)?,
            })
        })
        .await
        .ok()
        .or_forward(())
    }
}

pub struct AeadUserInfo {
    pub info: UserInfo,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AeadUserInfo {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let aead = try_outcome!(request.guard::<&State<AeadKey>>().await);
        let db = try_outcome!(request.guard::<DbConn>().await);

        let key = request
            .query_value::<String>("activation_key")
            .and_then(|x| x.ok());
        if let Some(key) = key {
            let decode_fn = || -> Result<String, anyhow::Error> {
                let decoded = base64::decode_config(&key, base64::URL_SAFE)?;
                Ok(String::from_utf8(aead.decrypt(&decoded).map_err(
                    |_| anyhow::anyhow!("mailaddress encryption failed"),
                )?)?)
            };

            let uid = decode_fn();

            db.run(move |c| -> Result<AeadUserInfo, anyhow::Error> {
                Ok(AeadUserInfo {
                    info: UserFinder::new(c, None).id(&uid?).first_info()?,
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
