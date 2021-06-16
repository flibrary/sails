use crate::{guards::*, wrap_op, DbConn, Msg};
use askama::Template;
use rocket::{
    form::Form,
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{
    messages::{Message, Messages},
    users::*,
};

// Form used for sending messages
#[derive(FromForm)]
pub struct SendMessage {
    body: String,
}

#[post("/send", data = "<info>")]
pub async fn send(
    user: UserIdGuard,
    receiver: UserIdParamGuard,
    info: Form<SendMessage>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let receiver_id = receiver.id.clone();
    wrap_op(
        conn.run(move |c| Messages::send(c, &user.id, &receiver.id, &info.body))
            .await,
        uri!("/messages"),
    )?;
    Ok(Redirect::to(format!(
        "/messages/chat?user_id={}#draft_section",
        receiver_id.get_id()
    )))
}

#[derive(Template)]
#[template(path = "messages/chat.html")]
pub struct ChatPage {
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

#[get("/chat", rank = 1)]
pub async fn chat(
    conn: DbConn,
    user: UserIdGuard,
    receiver: UserInfoParamGuard,
) -> Result<ChatPage, Flash<Redirect>> {
    let receiver_id = receiver.info.to_id();
    let messages = wrap_op(
        conn.run(move |c| Messages::get_conv(c, &user.id, &receiver_id))
            .await,
        uri!("/"),
    )?;
    Ok(ChatPage {
        messages,
        receiver: receiver.info,
    })
}

#[derive(Template)]
#[template(path = "messages/portal.html")]
pub struct PortalPage {
    message_list: Vec<Message>,
    inner: Msg,
}

#[get("/")]
pub async fn portal(
    flash: Option<FlashMessage<'_>>,
    user: Option<UserIdGuard>,
    conn: DbConn,
) -> Result<PortalPage, Flash<Redirect>> {
    if let Some(user) = user.map(|u| u.id) {
        let message_list = wrap_op(
            conn.run(move |c| Messages::get_list(c, &user)).await,
            uri!("/"),
        )?;
        Ok(PortalPage {
            message_list,
            inner: Msg::from_flash(flash),
        })
    } else {
        Err(Flash::error(
            Redirect::to(uri!("/user", crate::user::signin)),
            "sign in to view messages",
        ))
    }
}
