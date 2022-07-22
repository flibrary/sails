use crate::{
    guards::*,
    utils::{aead::AeadKey, i18n::I18n},
    DbConn, IntoFlash,
};
use askama::Template;
use bytes::Bytes;
use chacha20poly1305::Nonce;
use chrono::NaiveDateTime;
use image::{DynamicImage, ImageOutputFormat, Rgb};
use lopdf::{self, xobject, Document};
use qrcode::QrCode;
use rand::{prelude::StdRng, RngCore, SeedableRng};
use reqwest::Response;
use rocket::{
    data::ToByteUnit,
    form::{self, error::ErrorKind, DataField, FromFormField},
    http::{ContentType, Header},
    response::{Flash, Redirect},
    State,
};
use sails_db::{
    digicons::*,
    error::SailsDbError,
    users::{UserFinder, UserInfo},
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DigiconHosting {
    pub digicon_gh_token: String,
}

pub struct DigiconFile {
    pub bytes: Bytes,
    pub ctt_type: ContentType,
}

#[rocket::async_trait]
impl<'r> FromFormField<'r> for DigiconFile {
    async fn from_data(field: DataField<'r, '_>) -> form::Result<'r, Self> {
        // Picture, PDF, or zip archive
        if !(field.content_type.is_jpeg()
            || field.content_type.is_png()
            || field.content_type.is_pdf()
            || field.content_type.is_zip())
        {
            return Err(ErrorKind::Unexpected.into());
        }

        let limit = field
            .request
            .limits()
            .get("file")
            .unwrap_or_else(|| 5.mebibytes());

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

impl DigiconFile {
    pub async fn from_response(
        resp: Response,
        ctt_type: ContentType,
        aead: &AeadKey,
        trace: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            bytes: if ctt_type == ContentType::PDF {
                watermark_pdf(&resp.bytes().await?, aead, trace)?.into()
            } else {
                resp.bytes().await?
            },
            // Use the file extension to indicate the content type;
            // If no file extension is indicated, we use Any which works pretty well.
            ctt_type,
        })
    }
}

impl<'r, 'o: 'r> rocket::response::Responder<'r, 'o> for DigiconFile {
    fn respond_to(self, _: &'r rocket::request::Request<'_>) -> rocket::response::Result<'o> {
        rocket::response::Response::build()
            .header(self.ctt_type)
            // 2 minutes
            .header(Header::new("Cache-Control", "max-age=120"))
            .sized_body(self.bytes.len(), Cursor::new(self.bytes))
            .ok()
    }
}

#[derive(Template)]
#[template(path = "digicons/trace.html")]
pub struct TraceInfo {
    i18n: I18n,
    time: NaiveDateTime,
    digicon: Digicon,
    user: UserInfo,
}

#[get("/trace", rank = 2)]
pub async fn trace_unauthorized() -> Redirect {
    Redirect::to(uri!("https://flibrary.info/store", crate::store::home_page))
}

#[get("/trace?<cipher>&<nonce>", rank = 1)]
pub async fn trace(
    i18n: I18n,
    _auth: Role<Admin>,
    aead: &State<AeadKey>,
    cipher: String,
    nonce: String,
    db: DbConn,
) -> Result<TraceInfo, Flash<Redirect>> {
    let cipher = base64::decode_config(&cipher, base64::URL_SAFE).into_flash(uri!("/"))?;
    let nonce = Nonce::clone_from_slice(
        &base64::decode_config(&nonce, base64::URL_SAFE).into_flash(uri!("/"))?,
    );

    let decrypted: Vec<String> =
        String::from_utf8(aead.decrypt(&cipher, &nonce).into_flash(uri!("/"))?)
            .into_flash(uri!("/"))?
            .split(':')
            .map(|x| x.to_string())
            .collect();

    let time = NaiveDateTime::from_timestamp(decrypted[0].parse().into_flash(uri!("/"))?, 0);

    let (digicon, user) = db
        .run(move |c| -> Result<_, SailsDbError> {
            Ok((
                Digicons::find_by_id(c, &decrypted[1])?,
                UserFinder::new(c, None).id(&decrypted[2]).first_info()?,
            ))
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(TraceInfo {
        i18n,
        time,
        digicon,
        user,
    })
}

// Watermark using QRcode containing the AEAD encrypted trace string.
//
// What we are protecting against:
// without obtaining another legiminate copy of the same digicon downloaded by another user A, the malicious user cannot pretend the copy is downloaded by user A.
fn watermark_pdf(bytes: &[u8], aead: &AeadKey, trace: &str) -> anyhow::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut doc = Document::load_from(bytes)?;

    let mut rng = StdRng::from_entropy();

    // Generate nonce
    let mut nonce = [0u8; 12];
    rng.fill_bytes(&mut nonce);
    let nonce = Nonce::clone_from_slice(&nonce);

    // Encrypt trace with random nonce
    let ciphertext = base64::encode_config(
        aead.encrypt(trace.as_bytes(), &nonce)
            .map_err(|_| anyhow::anyhow!("digicon trace encryption failed"))?,
        base64::URL_SAFE,
    );

    // Base64 encode nonce
    let nonce = base64::encode_config(&nonce, base64::URL_SAFE);

    let mut buf = Vec::new();
    // IMPORTANT: Luma<u8> doesn't seem to work, while Rgb does
    let qrcode = DynamicImage::ImageRgb8(
        QrCode::new(uri!("https://flibrary.info/digicons", trace(ciphertext, nonce)).to_string())?
            .render::<Rgb<u8>>()
            .dark_color(Rgb([125u8, 125u8, 125u8]))
            .quiet_zone(false) // disable quiet zone (white border)
            .build(),
    );
    qrcode.write_to(&mut buf, ImageOutputFormat::Png)?;

    let stream = xobject::image_from(buf)?;

    for (_, page_id) in doc.get_pages() {
        doc.insert_image(page_id, stream.clone(), (10.0, 10.0), (75.0, 75.0))?;
    }

    doc.compress();
    doc.save_to(&mut result)?;
    Ok(result)
}
