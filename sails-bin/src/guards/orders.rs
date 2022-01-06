use crate::DbConn;
use rocket::{
    outcome::{try_outcome, IntoOutcome, Outcome},
    request::FromRequest,
};
use sails_db::{error::SailsDbError, products::*, transactions::*};

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
