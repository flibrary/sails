use crate::{infras::aead::AeadKey, pages::digicons::*};
use aws_sdk_s3::{
    client::Builder, middleware::DefaultMiddleware, Client, Config, Credentials, Endpoint, Region,
};
use bytes::Bytes;
use chacha20poly1305::Nonce;
use image::{ColorType, DynamicImage, ImageOutputFormat, Rgb};
use lopdf::{self, Document, ObjectId};
use qrcode::QrCode;
use rand::{prelude::StdRng, RngCore, SeedableRng};
use rocket::{
    data::ToByteUnit,
    fairing::{AdHoc, Fairing},
    figment::Figment,
    form::{self, error::ErrorKind, DataField, FromFormField},
    http::{ContentType, Header},
};
use std::io::Cursor;

#[derive(Clone, Debug)]
pub struct DigiconHosting {
    pub gh_token: String,
    pub s3_client: Client,
    pub s3_bucket: String,
}

impl DigiconHosting {
    fn from_figment(figment: &Figment) -> Result<Self, anyhow::Error> {
        #[derive(serde::Deserialize)]
        struct S3Config {
            endpoint: String,
            region: String,
            access_key: String,
            secret_key: String,
            bucket: String,
        }

        let gh_token: String = figment.extract_inner("digicons.gh_token")?;

        let config: S3Config = figment.extract_inner("digicons.s3")?;

        // One has to define something to be the credential provider name,
        // but it doesn't seem like the value matters
        let provider_name = "custom";
        let creds = Credentials::new(
            &config.access_key,
            &config.secret_key,
            None,
            None,
            provider_name,
        );

        let endpoint = Endpoint::immutable(config.endpoint.parse().unwrap());

        let mut client = Builder::dyn_https().middleware(DefaultMiddleware::new());
        client.set_sleep_impl(None);
        let client = client.build_dyn();

        let s3_config = Config::builder()
            .region(Region::new(config.region))
            .endpoint_resolver(endpoint)
            .credentials_provider(creds)
            .build();

        Ok(Self {
            gh_token,
            s3_client: Client::with_config(client, s3_config),
            s3_bucket: config.bucket,
        })
    }

    pub fn fairing() -> impl Fairing {
        AdHoc::try_on_ignite("digicons", move |rocket| async move {
            let hosting = match Self::from_figment(rocket.figment()) {
                Ok(c) => c,
                Err(e) => {
                    log::error!(
                        "Failed on constructing Digital Content hosting backend: {:?}",
                        e
                    );
                    return Err(rocket);
                }
            };
            Ok(rocket.manage(hosting))
        })
    }
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
    pub fn from_response(
        resp: Bytes,
        ctt_type: ContentType,
        aead: &AeadKey,
        trace: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            bytes: if ctt_type == ContentType::PDF {
                watermark_pdf(&resp, aead, trace)?.into()
            } else {
                resp
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

    // We use grayscale (to mimic alpha channel) and smask to implement transparent QR code.

    // IMPORTANT: Luma<u8> doesn't seem to work, while Rgb does
    let qrcode = DynamicImage::ImageRgb8(
        QrCode::new(uri!("https://flibrary.info/digicons", trace(ciphertext, nonce)).to_string())?
            .render::<Rgb<u8>>()
            // The grayscale is not the alpha channel, it is computed from all the RGB color channels.
            // Therefore, unless it is set to [0u8; 3], it is not gonna be transparent.
            // More info: https://stackoverflow.com/questions/42516203/converting-rgba-image-to-grayscale-golang
            .light_color(Rgb([0u8; 3]))
            .dark_color(Rgb([150u8, 150u8, 150u8]))
            .quiet_zone(false) // disable quiet zone (white border)
            .build(),
    );
    qrcode.write_to(&mut buf, ImageOutputFormat::Png)?;

    let (rgb, mask) = ImageXObject::from_rgb(&buf);
    let mask_id = doc.add_object(mask.into_object_with_mask(None));

    for (_, page_id) in doc.get_pages() {
        let stream = rgb.clone().into_object_with_mask(Some(mask_id));

        doc.insert_image(page_id, stream, (10.0, 10.0), (75.0, 75.0))?;
    }

    doc.compress();
    doc.save_to(&mut result)?;
    Ok(result)
}

#[derive(Clone)]
struct ImageXObject {
    /// Width of the image (original width, not scaled width)
    pub width: u32,
    /// Height of the image (original height, not scaled height)
    pub height: u32,
    /// Color space (Greyscale, RGB, CMYK)
    pub color_space: ColorType,
    /// The actual data from the image
    pub data: Vec<u8>,
}

impl ImageXObject {
    fn from_rgb(data: &[u8]) -> (Self, Self) {
        use image::GenericImageView;

        let img = image::load_from_memory(data).unwrap();

        let (width, height) = img.dimensions();

        let rgb = img.to_rgb8().to_vec();
        let gray = img.to_luma8().to_vec();

        (
            Self {
                width,
                height,
                color_space: img.color(),
                data: rgb,
            },
            Self {
                width,
                height,
                // We need to have it converted to "DeviceGray"
                color_space: ColorType::La8,
                data: gray,
            },
        )
    }

    fn into_object_with_mask(self, mask_id: Option<ObjectId>) -> lopdf::Stream {
        use lopdf::{Dictionary, Object, Object::*, Stream};

        let color_space = match self.color_space {
            ColorType::La8 => b"DeviceGray".to_vec(),
            ColorType::Rgb8 => b"DeviceRGB".to_vec(),
            ColorType::Rgb16 => b"DeviceRGB".to_vec(),
            ColorType::La16 => b"DeviceN".to_vec(),
            ColorType::Rgba8 => b"DeviceN".to_vec(),
            ColorType::Rgba16 => b"DeviceN".to_vec(),
            ColorType::Bgr8 => b"DeviceN".to_vec(),
            ColorType::Bgra8 => b"DeviceN".to_vec(),
            _ => b"Indexed".to_vec(),
        };

        let mut dict = Dictionary::new();
        dict.set("Type", Object::Name(b"XObject".to_vec()));
        dict.set("Subtype", Object::Name(b"Image".to_vec()));
        dict.set("Width", self.width);
        dict.set("Height", self.height);
        dict.set("ColorSpace", Object::Name(color_space));
        dict.set("BitsPerComponent", 8);

        if let Some(mask_id) = mask_id {
            dict.set("SMask", Reference(mask_id));
        }

        let mut img_object = Stream::new(dict, self.data);
        // Ignore any compression error.
        let _ = img_object.compress();
        img_object
    }
}
