use crate::{infras::guards::*, pages::msgs::*, DbConn, IntoFlash};
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};
use sails_db::messages::Messages;

// Form used for sending messages
#[derive(FromForm)]
pub struct SendMessage {
    body: String,
}

#[post("/send?<user_id>", data = "<info>")]
pub async fn send(
    user: UserIdGuard<Cookie>,
    user_id: UserGuard,
    info: Form<SendMessage>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let receiver = user_id.to_id_param(&conn).await.into_flash(uri!("/"))?;

    let receiver_id = receiver.id.clone();
    conn.run(move |c| Messages::send(c, &user.id, &receiver.id, &info.body))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!(
        "/messages",
        chat(receiver_id.get_id()),
        "#draft_section"
    )))
}
