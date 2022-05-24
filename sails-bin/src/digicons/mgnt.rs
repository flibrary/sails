use super::*;
use crate::{aead::AeadKey, guards::*, DbConn, IntoFlash};
use chrono::Utc;
use reqwest::header::ACCEPT;
use rocket::{
    form::Form,
    http::{ContentType, Status},
    response::{Flash, Redirect},
    State,
};
use sails_db::{digicons::*, enums::StorageType};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

#[post("/cow_digicon?<digicon_id>", data = "<info>", rank = 1)]
pub async fn update_digicon(
    digicon_id: DigiconGuard,
    _auth: Auth<DigiconWritable>,
    info: Form<DigiconUpdate>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    let id = digicon.get_id().to_string();
    conn.run(move |c| digicon.update_info(c, info.into_inner()))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/digicons", super::digicon_page(id))))
}

#[post("/cow_digicon", data = "<info>", rank = 2)]
pub async fn create_digicon(
    user: UserIdGuard<Cookie>,
    _auth: Auth<CanCreateDigicon>,
    info: Form<IncompleteDigicon>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| info.into_inner().create(c, &user.id))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/digicons")))
}

#[get("/get?<digicon_id>")]
pub async fn get(
    user: UserIdGuard<Cookie>,
    _auth: Auth<DigiconReadable>,
    hosting: &State<DigiconHosting>,
    digicon_id: DigiconGuard,
    aead: &State<AeadKey>,
    conn: DbConn,
) -> Result<Result<DigiconFile, Redirect>, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    // Generate the trace string using UTC time, digicon ID, and the user ID.
    // These are the information that we don't want adversary to tamper with.
    let trace = format!(
        "{}:{}:{}",
        Utc::now().timestamp(),
        digicon.get_id(),
        &user.id.get_id(),
    );
    let name = digicon.get_name().to_string();

    let client = reqwest::Client::builder()
        .user_agent("curl")
        .build()
        .unwrap();

    let storage_detail = match digicon.get_storage_detail() {
        None => return Ok(Err(Redirect::to(uri!("/static/404.html")))),
        Some(s) => s,
    };

    let link = match digicon.get_storage_type() {
        StorageType::GitRepo => {
            format!(
                "https://raw.githubusercontent.com/flibrary/digicons/main/{}",
                digicon.get_id()
            )
        }
        StorageType::ReleaseAsset => {
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
                .find(|x| x.name == storage_detail)
                .ok_or("asset not found in release")
                .into_flash(uri!("/"))?;

            asset.url
        }
    };

    let resp = client
        .get(link)
        .header(ACCEPT, "application/octet-stream")
        .bearer_auth(&hosting.digicon_gh_token)
        .send()
        .await
        .into_flash(uri!("/"))?;

    let ctt_type = std::path::Path::new(&name)
        .extension()
        .map(|x| ContentType::from_extension(x.to_str().unwrap()).unwrap())
        // If there is no extension, we set content type to any
        .unwrap_or(ContentType::Any);

    if resp.status().is_success() {
        // If we have successfully retrieved the file
        Ok(Ok(DigiconFile::from_response(resp, ctt_type, aead, &trace)
            .await
            .into_flash(uri!("/"))?))
    } else {
        Ok(Err(Redirect::to(uri!("/static/404.html"))))
    }
}

#[get("/delete?<digicon_id>")]
pub async fn delete(
    _auth: Auth<DigiconRemovable>,
    hosting: &State<DigiconHosting>,
    digicon_id: DigiconGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;

    match digicon.get_storage_type() {
        StorageType::GitRepo if digicon.get_storage_detail().is_some() => {
            let params = json!({
                "message": format!("delete: {} - {}", digicon.get_name(), digicon.get_id()),
                "sha": digicon.get_storage_detail().unwrap(),
            });

            let client = reqwest::Client::builder()
                .user_agent("curl")
                .build()
                .unwrap();

            client
                .delete(format!(
                    "https://api.github.com/repos/flibrary/digicons/contents/{}",
                    digicon.get_id(),
                ))
                .header(ACCEPT, "application/vnd.github.v3+json")
                .bearer_auth(&hosting.digicon_gh_token)
                .json(&params)
                .send()
                .await
                .into_flash(uri!("/"))?
                .error_for_status()
                .into_flash(uri!("/"))?;
        }
        _ => {}
    }

    conn.run(move |c| digicon.delete(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/digicons")))
}

#[post("/upload?<digicon_id>", data = "<file>")]
pub async fn upload(
    _auth: Auth<CanCreateDigicon>,
    db: DbConn,
    digicon_id: DigiconGuard,
    hosting: &State<DigiconHosting>,
    file: Form<DigiconFile>,
) -> Result<Redirect, Status> {
    let digicon = digicon_id
        .to_digicon(&db)
        .await
        .map_err(|_| Status::new(502))?;

    if !matches!(digicon.get_storage_type(), StorageType::GitRepo) {
        // Method not allowed
        return Err(Status::new(405));
    }

    use sha1::{Digest, Sha1};

    // Calculate the hash of the file
    let mut hasher = Sha1::new();
    // Git magic
    hasher.update(format!("blob {}\x00", file.bytes.len()));
    hasher.update(&file.bytes);
    let hash = hasher.finalize();

    let id = digicon.get_id().to_string();

    let client = reqwest::Client::builder()
        .user_agent("curl")
        .build()
        .unwrap();

    let params = match digicon.get_storage_detail() {
        // Creating new file
        None => json!({
            "content": base64::encode(&file.bytes),
            "message": format!("update: {} - {}", digicon.get_name(), digicon.get_id()),
        }),
        // Replacing old file
        Some(old_hash) => json!({
            "content": base64::encode(&file.bytes),
            "message": format!("update: {} - {}", digicon.get_name(), digicon.get_id()),
            "sha": old_hash,
        }),
    };
    // If not successful, we have to return wrong types
    if !client
        .put(format!(
            "https://api.github.com/repos/flibrary/digicons/contents/{}",
            digicon.get_id(),
        ))
        .header(ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&hosting.digicon_gh_token)
        .json(&params)
        .send()
        .await
        .map_err(|_| Status::new(502))?
        .status()
        .is_success()
    {
        // Bad Gateway
        return Err(Status::new(502));
    }

    db.run(move |c| {
        digicon
            .set_storage_detail(Some(format!("{:x}", hash)))
            .update(c)
    })
    .await
    // Internal Server Error
    .map_err(|_| Status::new(500))?;
    Ok(Redirect::to(uri!("/digicons", super::digicon_page(id))))
}
