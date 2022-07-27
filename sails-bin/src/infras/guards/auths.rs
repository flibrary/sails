use super::{digicons::*, orders::*, prods::*, users::*};
use crate::DbConn;
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
};
use sails_db::{
    digicons::DigiconMappingFinder,
    enums::{Payment, StorageType, UserStatus},
};
use std::marker::PhantomData;

// Misc
pub struct OrderWithPaypal;
pub struct OrderWithAlipay;
pub struct TagWritable;
pub struct CanCreateProduct;
pub struct CanCreateDigicon;

// For prods
pub struct ProdReadable;
pub struct ProdWritable;
pub struct ProdRemovable;
pub struct ProdAdmin;

// For users
pub struct UserReadable;
pub struct UserWritable;

// For orders
pub struct OrderReadable;
pub struct OrderProgressable;
pub struct OrderFinishable;
pub struct OrderRefundable;

// For digicons
pub struct DigiconReadable;
pub struct DigiconWritable;
pub struct DigiconRemovable;
pub struct DigiconContentReadable;
pub struct DigiconStorageType<T> {
    plhdr: PhantomData<T>,
}

// Storage Type
pub struct GitRepo;
pub struct ReleaseAsset;
pub struct S3;

pub struct Auth<T> {
    plhdr: PhantomData<T>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<OrderWithAlipay> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if *order.order_info.get_payment() == Payment::Alipay {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<OrderWithPaypal> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if *order.order_info.get_payment() == Payment::Paypal {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<TagWritable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        if user
            .info
            .get_user_status()
            .contains(UserStatus::TAG_WRITABLE)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<CanCreateProduct> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        if user
            .info
            .get_user_status()
            .contains(UserStatus::PROD_SELF_WRITABLE)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<CanCreateDigicon> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserInfoGuard<Cookie>>().await);

        if user
            .info
            .get_user_status()
            .contains(UserStatus::DIGICON_SELF_WRITABLE)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

// Prods
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<ProdAdmin> {
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
impl<'r> FromRequest<'r> for Auth<DigiconContentReadable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let digicon = try_outcome!(request
            .query_value::<DigiconGuard>("digicon_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let digicon = try_outcome!(digicon.to_digicon(&db).await.ok().or_forward(()));

        if db
            .run(move |c| DigiconMappingFinder::content_readable(c, &user.id, &digicon))
            .await
            .unwrap_or(false)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<DigiconReadable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let digicon = try_outcome!(request
            .query_value::<DigiconGuard>("digicon_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let digicon = try_outcome!(digicon.to_digicon(&db).await.ok().or_forward(()));

        if db
            .run(move |c| digicon.readable(c, &user.id))
            .await
            .unwrap_or(false)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<DigiconStorageType<GitRepo>> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let digicon = try_outcome!(request
            .query_value::<DigiconGuard>("digicon_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let digicon = try_outcome!(digicon.to_digicon(&db).await.ok().or_forward(()));

        if *digicon.get_storage_type() == StorageType::GitRepo {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<DigiconStorageType<ReleaseAsset>> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let digicon = try_outcome!(request
            .query_value::<DigiconGuard>("digicon_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let digicon = try_outcome!(digicon.to_digicon(&db).await.ok().or_forward(()));

        if *digicon.get_storage_type() == StorageType::ReleaseAsset {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<DigiconStorageType<S3>> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let digicon = try_outcome!(request
            .query_value::<DigiconGuard>("digicon_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let digicon = try_outcome!(digicon.to_digicon(&db).await.ok().or_forward(()));

        if *digicon.get_storage_type() == StorageType::S3 {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<DigiconWritable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let digicon = try_outcome!(request
            .query_value::<DigiconGuard>("digicon_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let digicon = try_outcome!(digicon.to_digicon(&db).await.ok().or_forward(()));

        if db
            .run(move |c| digicon.writable(c, &user.id))
            .await
            .unwrap_or(false)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<DigiconRemovable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let digicon = try_outcome!(request
            .query_value::<DigiconGuard>("digicon_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let digicon = try_outcome!(digicon.to_digicon(&db).await.ok().or_forward(()));

        if db
            .run(move |c| digicon.removable(c, &user.id))
            .await
            .unwrap_or(false)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<ProdReadable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let prod = try_outcome!(request
            .query_value::<ProdGuard>("prod_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let prod = try_outcome!(prod.to_info(&db).await.ok().or_forward(()));

        if db
            .run(move |c| prod.prod_info.readable(c, &user.id))
            .await
            .unwrap_or(false)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<ProdWritable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let prod = try_outcome!(request
            .query_value::<ProdGuard>("prod_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let prod = try_outcome!(prod.to_info(&db).await.ok().or_forward(()));

        if db
            .run(move |c| prod.prod_info.writable(c, &user.id))
            .await
            .unwrap_or(false)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth<ProdRemovable> {
    type Error = ();

    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let prod = try_outcome!(request
            .query_value::<ProdGuard>("prod_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let prod = try_outcome!(prod.to_info(&db).await.ok().or_forward(()));

        if db
            .run(move |c| prod.prod_info.removable(c, &user.id))
            .await
            .unwrap_or(false)
        {
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
            .and_then(|x| x.ok())
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
            .and_then(|x| x.ok())
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
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if db
            .run(move |c| order.order_info.readable(c, &user.id))
            .await
            .unwrap_or(false)
        {
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
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if db
            .run(move |c| order.order_info.progressable(c, &user.id))
            .await
            .unwrap_or(false)
        {
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
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if db
            .run(move |c| order.order_info.finishable(c, &user.id))
            .await
            .unwrap_or(false)
        {
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
        let user = try_outcome!(request.guard::<UserIdGuard<Cookie>>().await);

        let db = try_outcome!(request.guard::<DbConn>().await);
        let order = try_outcome!(request
            .query_value::<OrderGuard>("order_id")
            .and_then(|x| x.ok())
            .or_forward(()));
        let order = try_outcome!(order.to_info(&db).await.ok().or_forward(()));

        if db
            .run(move |c| order.order_info.refundable(c, &user.id))
            .await
            .unwrap_or(false)
        {
            Outcome::Success(Auth { plhdr: PhantomData })
        } else {
            Outcome::Forward(())
        }
    }
}
