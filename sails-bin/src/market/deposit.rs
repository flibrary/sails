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
use sails_db::{enums::ProductStatus, products::*};

#[derive(Template)]
#[template(path = "market/deposit.html")]
pub struct DepositInfo {
    book: ProductInfo,
    // Alipay precreate API response
    resp: Option<Result<PrecreateResp, SignedResponse<PrecreateResp>>>,
}

#[get("/deposit_info")]
pub async fn deposit_info(
    // This page contains progressable information
    // TODO: this is not a good enough distinguishment
    _auth: Auth<BookWritable>,
    book: BookInfoGuard<ProductInfo>,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<DepositInfo, Flash<Redirect>> {
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
                    50,
                ),
            )
            .into_flash(uri!("/"))?
            .send::<PrecreateResp>()
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
#[get("/deposit_progress", rank = 1)]
pub async fn deposit_progress(
    _auth: Auth<BookWritable>,
    book: BookInfoGuard<MutableProductInfo>,
    db: DbConn,
    priv_key: &State<AlipayAppPrivKey>,
    client: &State<AlipayClient>,
) -> Result<Redirect, Flash<Redirect>> {
    let id = book.book_info.get_id().to_string();

    let resp = client
        .request(priv_key, TradeQuery::new(book.book_info.get_id()))
        .into_flash(uri!("/"))?
        .send::<TradeQueryResp>()
        .await
        .into_flash(uri!("/"))?
        .into_flash(uri!("/"))?;

    // If the product status is abnormal, this progress update should not allow users to populate their books.
    if ((resp.trade_status == "TRADE_SUCCESS") | (resp.trade_status == "TRADE_FINISHED"))
        && (book.book_info.get_product_status() == &ProductStatus::Normal)
    {
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
    }
    Ok(Redirect::to(format!("/market/deposit_info?book_id={}", id)))
}
