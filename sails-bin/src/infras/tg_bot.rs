use crate::{
    infras::guards::{OrderGuard, ProdGuard, UserGuard},
    DbConn,
};
use sails_db::{products::ProductInfo, transactions::TransactionInfo, users::UserInfo};
use serde::{Deserialize, Deserializer};
use std::{future::Future, sync::Arc, time::Duration};
use teloxide::{adaptors::DefaultParseMode, prelude::*};

fn deserialize_bot_token<'de, D>(
    deserializer: D,
) -> Result<Arc<AutoSend<DefaultParseMode<Bot>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let token = String::deserialize(deserializer)?;

    Ok(Arc::new(
        DefaultParseMode::new(Bot::new(token), teloxide::types::ParseMode::MarkdownV2).auto_send(),
    ))
}

#[derive(Clone, Deserialize)]
pub struct TelegramBot {
    #[serde(deserialize_with = "deserialize_bot_token")]
    pub bot_token: Arc<AutoSend<DefaultParseMode<Bot>>>,
    pub channel_id: i64,
}

impl TelegramBot {
    pub async fn send_order_update(&self, id: impl ToString, conn: &DbConn) -> anyhow::Result<()> {
        let order = OrderGuard::new(id).to_info(conn).await?;
        let buyer = order.buyer_info;
        let seller = order.seller_info;
        let product = order.prod_info;
        let order = order.order_info;

        // In all other places characters '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!' must be escaped with the preceding character '\'.
        let escape = |u: &str| {
            u.replace('_', "\\_")
                .replace('*', "\\*")
                .replace('[', "\\[")
                .replace(']', "\\]")
                .replace('(', "\\(")
                .replace(')', "\\)")
                .replace('~', "\\~")
                .replace('`', "\\`")
                .replace('>', "\\>")
                .replace('#', "\\#")
                .replace('+', "\\+")
                .replace('-', "\\-")
                .replace('=', "\\=")
                .replace('|', "\\|")
                .replace('{', "\\{")
                .replace('}', "\\}")
                .replace('.', "\\.")
                .replace('!', "\\!")
        };

        let user_link = |u: &UserInfo| {
            format!(
                "[{}]({})",
                escape(u.get_name()),
                uri!(
                    "https://flibrary.info/user",
                    crate::pages::users::portal_guest(u.get_id())
                )
            )
        };
        let product_link = |p: &ProductInfo| {
            format!(
                "[{}]({})",
                escape(p.get_prodname()),
                uri!(
                    "https://flibrary.info/store",
                    crate::pages::store::prod_page_guest(p.get_id())
                )
            )
        };
        let order_link = |t: &TransactionInfo| {
            format!(
                "[\\#{}]({})",
                escape(t.get_shortid()),
                uri!(
                    "https://flibrary.info/admin",
                    crate::pages::admin::order_info(t.get_id())
                )
            )
        };

        let msg = format!(
            r#"Order Status Update: *\#{:?}*:
Order: {order}
Buyer: {buyer}
Seller: {seller}
Product: {product}
Coupon: {coupon}
Price: {price}
Quantity: {qty}
Subtotal: {subtotal}
Discount: {discount}
Total: {total}"#,
            order.get_transaction_status(),
            order = order_link(&order),
            buyer = user_link(&buyer),
            seller = user_link(&seller),
            coupon = escape(order.get_coupon()),
            product = product_link(&product),
            price = order.get_price(),
            qty = order.get_quantity(),
            subtotal = order.get_subtotal(),
            discount = order.get_discount(),
            total = order.get_total()
        );

        let bot_token = self.bot_token.clone();
        let channel_id = self.channel_id;
        // I don't know why it redundantly sends update sometimes.
        tokio::spawn(async move {
            // Discard the error
            tryn(5, Duration::from_secs(5), || {
                bot_token
                    .send_message(ChatId(channel_id), msg.clone())
                    .disable_web_page_preview(true)
            })
            .await
            .map(drop)
            .unwrap_or_else(|err| {
                error_!(
                    "telegram bot failed to send notification of placed order {} to chat {}: {}",
                    order.get_shortid(),
                    channel_id,
                    err
                )
            });
        });
        Ok(())
    }
}

pub async fn tryn<F, Fut, T, E>(n: usize, delay: Duration, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    for _ in 1..n {
        if let ret @ Ok(_) = f().await {
            return ret;
        }

        tokio::time::sleep(delay).await;
    }

    f().await
}
