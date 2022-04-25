use crate::{guards::*, DbConn, IntoFlash};
use bytes::Bytes;
use reqwest::{header::ACCEPT, Response};
use rocket::{
    http::{ContentType, Header},
    response::{Flash, Redirect},
    State,
};
use sails_db::digicons::*;
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DigiconHosting {
    digicon_gh_token: String,
}

pub struct DigiconFile {
    pub bytes: Bytes,
    pub ctt_type: ContentType,
}

impl DigiconFile {
    pub async fn from_response(resp: Response, name: String) -> anyhow::Result<Self> {
        Ok(Self {
            // Use the file extension to indicate the content type;
            // If no file extension is indicated, we use Any which works pretty well.
            ctt_type: std::path::Path::new(&name)
                .extension()
                .map(|x| ContentType::from_extension(x.to_str().unwrap()).unwrap())
                .unwrap_or(ContentType::Any),
            bytes: resp.bytes().await?,
        })
    }
}

impl<'r, 'o: 'r> rocket::response::Responder<'r, 'o> for DigiconFile {
    fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'o> {
        rocket::response::Response::build()
            .header(self.ctt_type)
            // One day
            .header(Header::new("Cache-Control", "max-age=86400"))
            .sized_body(self.bytes.len(), Cursor::new(self.bytes))
            .ok()
    }
}

#[get("/get?<digicon_id>")]
pub async fn get(
    user: UserIdGuard<Cookie>,
    hosting: &State<DigiconHosting>,
    digicon_id: DigiconGuard,
    conn: DbConn,
) -> Result<Result<DigiconFile, Redirect>, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    let link = digicon.get_link().to_string();
    let name = digicon.get_name().to_string();
    if !conn
        .run(move |c| DigiconMappingFinder::is_authorized(&c, &user.id, &digicon))
        .await
        .into_flash(uri!("/"))?
    {
        return Ok(Err(Redirect::to(uri!("/static/404.html"))));
    }
    let client = reqwest::Client::builder()
        .user_agent("curl")
        .build()
        .unwrap();
    let resp = client
        .get(link)
        .header(ACCEPT, "application/octet-stream")
        // If we don't auth, probably we will get limited further
        .bearer_auth(&hosting.digicon_gh_token)
        .send()
        .await
        .into_flash(uri!("/"))?;

    if resp.status().is_success() {
        // If we have successfully retrieved the file
        Ok(Ok(DigiconFile::from_response(resp, name)
            .await
            .into_flash(uri!("/"))?))
    } else {
        Ok(Err(Redirect::to(uri!("/static/404.html"))))
    }
}
