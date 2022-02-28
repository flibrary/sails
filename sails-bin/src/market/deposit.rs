use crate::{
    alipay::{
        AlipayAppPrivKey, AlipayClient, Precreate, PrecreateResp, SignedResponse, TradeQuery,
        TradeQueryResp,
    },
    guards::*,
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::{
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    enums::ProductStatus,
    error::SailsDbError,
    products::*,
    tags::{TagMapping, Tags},
};

#[derive(Template)]
#[template(path = "market/deposit.html")]
pub struct DepositInfo {
    book: ProductInfo,
    // Alipay precreate API response
    resp: Option<Result<PrecreateResp, SignedResponse<PrecreateResp>>>,
}

#[get("/deposit_info?<book_id>")]
pub async fn deposit_info(
    // This page contains progressable information
    // TODO: this is not a good enough distinguishment
    _auth: Auth<BookWritable>,
    book_id: BookGuard,
    conn: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<DepositInfo, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    if book.book_info.get_product_status() == &ProductStatus::Normal {
        // It seems like we could request precreation even if the user has already paid the bill or the trade has already been created.
        // If, in the future, this behavior changes, we have to come up with a better mechanism.
        // Currently, if anything goes wrong, we would have the message for debug, and the next button would still be available.
        let resp = client
            .request(
                priv_key,
                Precreate::new(
                    book.book_info.get_id(),
                    &format!("Security Deposit: {}", book.book_info.get_prodname()),
                    50u32.into(),
                ),
            )
            .into_flash(uri!("/"))?
            .send::<PrecreateResp>(client.client())
            .await
            .into_flash(uri!("/"))?;
        Ok(DepositInfo {
            book: book.book_info,
            resp: Some(resp),
        })
    } else {
        Ok(DepositInfo {
            book: book.book_info,
            resp: None,
        })
    }
}

// Basically, we syncronize our trade status with that in alipay
#[get("/deposit_progress?<book_id>", rank = 1)]
pub async fn deposit_progress(
    _auth: Auth<BookWritable>,
    book_id: BookGuard,
    db: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_info(&db).await.into_flash(uri!("/"))?;

    let resp = client
        .request(priv_key, TradeQuery::new(book.book_info.get_id()))
        .into_flash(uri!("/"))?
        .send::<TradeQueryResp>(client.client())
        .await
        .into_flash(uri!("/"))?
        .into_flash(uri!("/"))?;

    // If the product status is abnormal, this progress update should not allow users to populate their books.
    if ((resp.trade_status == "TRADE_SUCCESS") | (resp.trade_status == "TRADE_FINISHED"))
        && (book.book_info.get_product_status() == &ProductStatus::Normal)
    {
        let id = book.book_info.to_id();
        // Both of these indicate that we have successfully finished the transaction.
        // TRADE_FINISHED indicates it has been well pass the refunding deadline.
        // This means we can now verify this book!
        db.run(move |c| {
            book.book_info
                .set_product_status(ProductStatus::Verified)
                .update(c)
        })
        .await
        .into_flash(uri!("/"))?;

        // Add the preorder tag automatically
        db.run(move |c| -> Result<(), SailsDbError> {
            let tag = Tags::find_by_id(c, "preorder")?;
            TagMapping::create(c, &tag, &id)?;
            Ok(())
        })
        .await
        .into_flash(uri!("/"))?;
    }
    Ok(Redirect::to(uri!("/market", deposit_info(book_id))))
}
