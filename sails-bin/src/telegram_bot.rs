use crate::guards::{BookGuard, UserGuard};
use sails_db::{products::ProductInfo, transactions::TransactionInfo, users::UserInfo};
use serde::{Deserialize, Deserializer};
use std::{future::Future, sync::Arc, time::Duration};
use teloxide::{adaptors::DefaultParseMode, prelude2::*};

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
    pub async fn send_order_placed(
        &self,
        order: &TransactionInfo,
        buyer: &UserInfo,
        seller: &UserInfo,
        product: &ProductInfo,
    ) {
        let user_link = |u: &UserInfo| {
            format!(
                "[{}]({})",
                u.get_name(),
                uri!(
                    "https://flibrary.info/user",
                    crate::user::portal_guest(u.get_id())
                )
            )
        };
        let product_link = |p: &ProductInfo| {
            format!(
                "[{}]({})",
                p.get_prodname(),
                uri!(
                    "https://flibrary.info/market",
                    crate::market::book_page_guest(p.get_id())
                )
            )
        };

        let msg = format!(
            r#"*New order placed*:
Buyer: {buyer}
Seller: {seller}
Product: {product}
Price: {price}
Quantity: {qty}
Total: {total}"#,
            buyer = user_link(buyer),
            seller = user_link(seller),
            product = product_link(product),
            price = order.get_price(),
            qty = order.get_quantity(),
            total = order.get_total()
        );
        // Discard the error
        tryn(5, Duration::from_secs(5), || {
            self.bot_token
                .send_message(self.channel_id, msg.clone())
                .disable_web_page_preview(true)
        })
        .await
        .map(drop)
        .unwrap_or_else(|err| {
            error_!(
                "telegram bot failed to send notification of placed order {} to chat {}: {}",
                order.get_shortid(),
                self.channel_id,
                err
            )
        });
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
