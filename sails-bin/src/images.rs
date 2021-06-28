use crate::guards::UserIdGuard;
use reqwest::header::ACCEPT;
use rocket::{
    data::ToByteUnit,
    form::{self, error::ErrorKind, DataField, Form, FromFormField},
    http::{ContentType, Status},
    response::Redirect,
    State,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageHosting {
    gh_token: String,
}

pub struct Image {
    pub bytes: Vec<u8>,
    pub ctt_type: ContentType,
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
            bytes,
            ctt_type: field.content_type,
        })
    }
}

#[get("/get/<hash>/<ext>/<size>", rank = 1)]
pub async fn get(hash: &str, ext: &str, size: &str) -> Redirect {
    Redirect::to(format!(
        "https://flibrary.lexugeyky.workers.dev/https://raw.githubusercontent.com/flibrary/images/main/{}/{}.{}",
        hash, size, ext
    ))
}

#[get("/get/<hash>/<ext>", rank = 2)]
pub async fn get_default(hash: &str, ext: &str) -> Redirect {
    Redirect::to(format!(
        "https://flibrary.lexugeyky.workers.dev/https://raw.githubusercontent.com/flibrary/images/main/{}/orig.{}",
        hash, ext
    ))
}

// We only allow signed in users to upload images
#[post("/upload", data = "<img>")]
pub async fn upload(
    _user: UserIdGuard,
    hosting: &State<ImageHosting>,
    img: Form<Image>,
) -> Result<String, Status> {
    use sha2::{Digest, Sha256};

    // Content types are restricted to jpeg and png, should be fine to unwrap.
    let ext = img.ctt_type.extension().unwrap();

    // Calculate the hash of the image
    let mut hasher = Sha256::new();
    hasher.update(&img.bytes);
    let hash = hasher.finalize();

    let client = reqwest::Client::builder()
        .user_agent("curl")
        .build()
        .unwrap();

    // If the content doesn't exist yet, we have to create it
    if !client
        .get(format!(
            "https://api.github.com/repos/flibrary/images/contents/{:x}/",
            hash,
        ))
        .header(ACCEPT, "application/vnd.github.v3+json")
        // If we don't auth, probably we will get limited further
        .bearer_auth(&hosting.gh_token)
        .send()
        .await
        .map_err(|_| Status::new(503))?
        .status()
        .is_success()
    {
        let params = json!({
            "content": base64::encode(&img.bytes),
            "message": format!("add: {:x}", hash),
        });
        // If not successful, we have to return wrong types
        if !client
            .put(format!(
                "https://api.github.com/repos/flibrary/images/contents/{:x}/orig.{}",
                hash, ext
            ))
            .header(ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&hosting.gh_token)
            .json(&params)
            .send()
            .await
            .map_err(|_| Status::new(502))?
            .status()
            .is_success()
        {
            return Err(Status::new(400));
        }
    }
    Ok(uri!("/images", get_default(format!("{:x}", hash), ext.as_str())).to_string())
}
