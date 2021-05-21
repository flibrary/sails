use crate::{
    guards::{ReceiverGuard, UserGuard},
    wrap_op, DbConn, Msg,
};
use askama::Template;
use rocket::{
    form::Form,
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{
    messages::{Message, Messages},
    users::User,
};

// Form used for sending messages
#[derive(FromForm)]
pub struct SendMessage {
    body: String,
}

#[post("/send", data = "<info>")]
pub async fn send(
    user: UserGuard,
    receiver: ReceiverGuard,
    info: Form<SendMessage>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let receiver_id = receiver.receiver.get_id().to_string();
    wrap_op(
        conn.run(move |c| {
            Messages::send(
                c,
                user.user.get_id(),
                receiver.receiver.get_id(),
                &info.body,
            )
        })
        .await,
        uri!("/messages"),
    )?;
    Ok(Redirect::to(format!(
        "/messages/chat?receiver_id={}#draft_section",
        receiver_id
    )))
}

#[derive(Template)]
#[template(path = "messages/chat.html")]
pub struct ChatPage {
    messages: Vec<Message>,
    receiver: User,
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
    user: UserGuard,
    receiver: ReceiverGuard,
) -> Result<ChatPage, Flash<Redirect>> {
    let receiver_id = receiver.receiver.get_id().to_string();
    let messages = wrap_op(
        conn.run(move |c| Messages::get_conv(c, user.user.get_id(), &receiver_id))
            .await,
        uri!("/"),
    )?;
    Ok(ChatPage {
        messages,
        receiver: receiver.receiver,
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
    user: Option<UserGuard>,
    conn: DbConn,
) -> Result<PortalPage, Flash<Redirect>> {
    if let Some(user) = user.map(|u| u.user) {
        let message_list = wrap_op(
            conn.run(move |c| Messages::get_list(c, user.get_id()))
                .await,
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
