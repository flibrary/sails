use crate::{
    infras::{guards::*, images::*},
    IntoFlash,
};
use reqwest::header::ACCEPT;
use rocket::{
    form::Form,
    http::Status,
    response::{Flash, Redirect},
    State,
};
use serde_json::json;

// Ok(Ok(Image)) we return image directly.
// Ok(Err(Redirect)) image unable to be found, return a redirection to a placeholder image.
// Err(Flash<Redirect>) error occured during fetching.
#[get("/get/<hash>/<ext>?<size>", rank = 1)]
pub async fn get(
    hash: &str,
    ext: &str,
    size: &str,
) -> Result<Result<Image, Redirect>, Flash<Redirect>> {
    let resp = reqwest::get(format!(
        "https://raw.githubusercontent.com/flibrary/images/main/{}/{}.{}",
        hash, size, ext
    ))
    .await
    .into_flash(uri!("/"))?;

    match resp.status().is_success() {
        // If we have successfully retrieved the image
        true => Ok(Ok(Image::from_response(resp)
            .await
            .into_flash(uri!("/"))?)),
        // If the size indication is not original, we may fallback to original quality
        false if size != "orig" => {
            if let Ok(orig_resp) = reqwest::get(format!(
                "https://raw.githubusercontent.com/flibrary/images/main/{}/{}.{}",
                hash, "orig", ext
            ))
            .await
            .into_flash(uri!("/"))
            {
                Ok(Ok(Image::from_response(orig_resp)
                    .await
                    .into_flash(uri!("/"))?))
            } else {
                Ok(Err(Redirect::to(uri!("/static/logo.png"))))
            }
        }
        // We have nothing to fallback, return placeholder image
        false if size == "orig" => Ok(Err(Redirect::to(uri!("/static/logo.png")))),
        false => unreachable!(),
    }
}

#[get("/get/<hash>/<ext>", rank = 2)]
pub async fn get_default(
    hash: &str,
    ext: &str,
) -> Result<Result<Image, Redirect>, Flash<Redirect>> {
    get(hash, ext, "orig").await
}

// We only allow signed in users to upload images
#[post("/upload", data = "<img>")]
pub async fn upload(
    _user: UserIdGuard<Cookie>,
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
