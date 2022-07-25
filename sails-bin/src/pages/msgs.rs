use crate::{
    infras::{guards::*, i18n::I18n},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{
    messages::{Message, Messages},
    users::*,
};

#[derive(Template)]
#[template(path = "messages/chat.html")]
pub struct ChatPage {
    i18n: I18n,
    messages: Vec<Message>,
    receiver: UserInfo,
}

#[get("/chat", rank = 2)]
pub async fn chat_error() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/messages")),
        "missing(invalid) receiver or you are not signed in",
    )
}

#[get("/chat?<user_id>", rank = 1)]
pub async fn chat(
    i18n: I18n,
    conn: DbConn,
    user: UserIdGuard<Cookie>,
    user_id: UserGuard,
) -> Result<ChatPage, Flash<Redirect>> {
    let receiver = user_id.to_info_param(&conn).await.into_flash(uri!("/"))?;

    let receiver_id = receiver.info.to_id();
    let messages = conn
        .run(move |c| Messages::get_conv(c, &user.id, &receiver_id))
        .await
        .into_flash(uri!("/"))?;
    Ok(ChatPage {
        i18n,
        messages,
        receiver: receiver.info,
    })
}

#[derive(Template)]
#[template(path = "messages/portal.html")]
pub struct PortalPage {
    i18n: I18n,
    message_list: Vec<Message>,
}

#[get("/")]
pub async fn portal(
    i18n: I18n,
    user: Option<UserIdGuard<Cookie>>,
    conn: DbConn,
) -> Result<PortalPage, Flash<Redirect>> {
    if let Some(user) = user.map(|u| u.id) {
        let message_list = conn
            .run(move |c| Messages::get_list(c, &user))
            .await
            .into_flash(uri!("/"))?;
        Ok(PortalPage { i18n, message_list })
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/")),
            "sign in to view messages",
        ))
    }
}
