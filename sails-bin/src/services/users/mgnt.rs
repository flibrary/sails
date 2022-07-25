use crate::{infras::guards::*, sanitize_html, DbConn, IntoFlash};
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};

#[derive(Debug, FromForm, Clone)]
pub struct PartialUserFormOwned {
    pub name: String,
    pub school: String,
    pub description: Option<String>,
}

#[post("/update_user", data = "<info>")]
pub async fn update_user(
    user: UserInfoGuard<Cookie>,
    info: Form<PartialUserFormOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let mut info = info.into_inner();
    info.description = info.description.map(|d| sanitize_html(&d));
    conn.run(move |c| {
        user.info
            .set_description(info.description)
            .set_name(info.name)
            .set_school(info.school)
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/user", crate::pages::users::portal)))
}
