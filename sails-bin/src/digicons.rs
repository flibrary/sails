use crate::{aead::AeadKey, guards::*, DbConn, IntoFlash};
use bytes::Bytes;
use chacha20poly1305::Nonce;
use image::{DynamicImage, ImageOutputFormat, Rgb};
use lopdf::{self, xobject, Document};
use qrcode::QrCode;
use rand::{prelude::StdRng, RngCore, SeedableRng};
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ReleaseAssets {
    assets: Vec<Asset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Asset {
    // The name of the release asset
    name: String,
    // Download endpoint
    url: String,
}

pub struct DigiconFile {
    pub bytes: Bytes,
    pub ctt_type: ContentType,
}

impl DigiconFile {
    pub async fn from_response(
        resp: Response,
        name: String,
        aead: &AeadKey,
        trace: &str,
    ) -> anyhow::Result<Self> {
        let ctt_type = std::path::Path::new(&name)
            .extension()
            .map(|x| ContentType::from_extension(x.to_str().unwrap()).unwrap())
            // If there is no extension, we set content type to any
            .unwrap_or(ContentType::Any);
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
            // One day
            .header(Header::new("Cache-Control", "max-age=86400"))
            .sized_body(self.bytes.len(), Cursor::new(self.bytes))
            .ok()
    }
}

#[get("/trace?<cipher>&<nonce>")]
pub async fn trace(
    _auth: Role<Admin>,
    aead: &State<AeadKey>,
    cipher: String,
    nonce: String,
) -> Result<String, Flash<Redirect>> {
    let cipher = base64::decode_config(&cipher, base64::URL_SAFE).into_flash(uri!("/"))?;
    let nonce = Nonce::clone_from_slice(
        &base64::decode_config(&nonce, base64::URL_SAFE).into_flash(uri!("/"))?,
    );

    String::from_utf8(aead.decrypt(&cipher, &nonce).into_flash(uri!("/"))?).into_flash(uri!("/"))
}

#[get("/get?<digicon_id>")]
pub async fn get(
    user: UserIdGuard<Cookie>,
    hosting: &State<DigiconHosting>,
    digicon_id: DigiconGuard,
    aead: &State<AeadKey>,
    conn: DbConn,
) -> Result<Result<DigiconFile, Redirect>, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    // Generate the trace string using digicon ID, and the user ID.
    // These are the information that we don't want adversary to tamper with.
    let trace = format!("{} - {}", digicon.get_id(), &user.id.get_id(),);
    let link = digicon.get_link().to_string();
    let name = digicon.get_name().to_string();
    if !conn
        .run(move |c| DigiconMappingFinder::is_authorized(c, &user.id, &digicon))
        .await
        .into_flash(uri!("/"))?
    {
        return Ok(Err(Redirect::to(uri!("/static/404.html"))));
    }

    let download_link = if let Some(filename) = link.strip_prefix("euler://") {
        let client = reqwest::Client::builder()
            .user_agent("curl")
            .build()
            .unwrap();
        // TODO: don't make it hardcoded
        let assets = client
            .get("https://api.github.com/repos/flibrary/euler/releases/latest")
            .header(ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&hosting.digicon_gh_token)
            .send()
            .await
            .into_flash(uri!("/"))?
            .json::<ReleaseAssets>()
            .await
            .into_flash(uri!("/"))?;
        let asset = assets
            .assets
            .into_iter()
            .find(|x| x.name == filename)
            .ok_or("asset not found in release")
            .into_flash(uri!("/"))?;
        asset.url
    } else {
        link
    };

    let client = reqwest::Client::builder()
        .user_agent("curl")
        .build()
        .unwrap();
    let resp = client
        .get(download_link)
        .header(ACCEPT, "application/octet-stream")
        // If we don't auth, probably we will get limited further
        .bearer_auth(&hosting.digicon_gh_token)
        .send()
        .await
        .into_flash(uri!("/"))?;

    if resp.status().is_success() {
        // If we have successfully retrieved the file
        Ok(Ok(DigiconFile::from_response(resp, name, aead, &trace)
            .await
            .into_flash(uri!("/"))?))
    } else {
        Ok(Err(Redirect::to(uri!("/static/404.html"))))
    }
}

// Watermark the producer field with AEAD encrypted trace string.
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
