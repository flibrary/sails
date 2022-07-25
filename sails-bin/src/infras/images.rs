use bytes::Bytes;
use reqwest::{header::CONTENT_TYPE, Response};
use rocket::{
    data::ToByteUnit,
    form::{self, error::ErrorKind, DataField, FromFormField},
    http::{ContentType, Header},
};
use serde::{Deserialize, Serialize};
use std::{io::Cursor, str::FromStr};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageHosting {
    pub gh_token: String,
}

pub struct Image {
    pub bytes: Bytes,
    pub ctt_type: ContentType,
}

impl Image {
    pub async fn from_response(resp: Response) -> anyhow::Result<Self> {
        Ok(Self {
            ctt_type: ContentType::from_str(
                resp.headers().get(CONTENT_TYPE).unwrap().to_str().unwrap(),
            )
            .unwrap(),
            bytes: resp.bytes().await?,
        })
    }
}

#[rocket::async_trait]
impl<'r> FromFormField<'r> for Image {
    async fn from_data(field: DataField<'r, '_>) -> form::Result<'r, Self> {
        if (!field.content_type.is_jpeg()) && (!field.content_type.is_png()) {
            return Err(ErrorKind::Unexpected.into());
        }

        let limit = field
            .request
            .limits()
            .get("image")
            .unwrap_or_else(|| 1.mebibytes());

        let bytes = field.data.open(limit).into_bytes().await?;
        if !bytes.is_complete() {
            return Err((None, Some(limit)).into());
        }
        let bytes = bytes.into_inner();
        Ok(Self {
            bytes: bytes.into(),
            ctt_type: field.content_type,
        })
    }
}

impl<'r, 'o: 'r> rocket::response::Responder<'r, 'o> for Image {
    fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'o> {
        rocket::response::Response::build()
            .header(self.ctt_type)
            .header(Header::new("Cache-Control", "max-age=31536000"))
            .sized_body(self.bytes.len(), Cursor::new(self.bytes))
            .ok()
    }
}
