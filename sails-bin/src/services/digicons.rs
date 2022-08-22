use crate::{
    infras::{aead::AeadKey, digicons::*, guards::*},
    pages::digicons::*,
    DbConn, IntoFlash,
};
use bytes::{BufMut, BytesMut};
use chrono::Utc;
use reqwest::header::ACCEPT;
use rocket::{
    form::Form,
    http::{ContentType, Status},
    response::{Flash, Redirect},
    State,
};
use sails_db::digicons::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Write;
use tokio_stream::StreamExt;

const MIN_CAPACITY: usize = 1024;

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
    Ok(Redirect::to(uri!("/digicons", digicon_page(id))))
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

#[get("/get?<digicon_id>", rank = 1)]
pub async fn get_release_asset(
    user: UserIdGuard<Cookie>,
    _auth: Auth<DigiconContentReadable>,
    _type: Auth<DigiconStorageType<ReleaseAsset>>,
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

    let client = reqwest::Client::builder()
        .user_agent("curl")
        .build()
        .unwrap();

    let storage_detail = match digicon.get_storage_detail() {
        None => return Ok(Err(Redirect::to(uri!("/static/404.html")))),
        Some(s) => s,
    };

    let link = {
        let assets = client
            .get("https://api.github.com/repos/flibrary/euler/releases/latest")
            .header(ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&hosting.gh_token)
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
    };

    let resp = client
        .get(link)
        .header(ACCEPT, "application/octet-stream")
        .bearer_auth(&hosting.gh_token)
        .send()
        .await
        .into_flash(uri!("/"))?;

    let ctt_type = std::path::Path::new(storage_detail)
        .extension()
        .map(|x| ContentType::from_extension(x.to_str().unwrap()).unwrap())
        // If there is no extension, we set content type to any
        .unwrap_or(ContentType::Any);

    if resp.status().is_success() {
        // If we have successfully retrieved the file
        Ok(Ok(DigiconFile::from_response(
            resp.bytes().await.into_flash(uri!("/"))?,
            ctt_type,
            aead,
            &trace,
        )
        .into_flash(uri!("/"))?))
    } else {
        Ok(Err(Redirect::to(uri!("/static/404.html"))))
    }
}

#[get("/get?<digicon_id>", rank = 2)]
pub async fn get_git_repo(
    user: UserIdGuard<Cookie>,
    _auth: Auth<DigiconContentReadable>,
    _type: Auth<DigiconStorageType<GitRepo>>,
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

    let link = format!(
        "https://raw.githubusercontent.com/flibrary/digicons/main/{}",
        digicon.get_id()
    );

    let resp = client
        .get(link)
        .header(ACCEPT, "application/octet-stream")
        .bearer_auth(&hosting.gh_token)
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
        Ok(Ok(DigiconFile::from_response(
            resp.bytes().await.into_flash(uri!("/"))?,
            ctt_type,
            aead,
            &trace,
        )
        .into_flash(uri!("/"))?))
    } else {
        Ok(Err(Redirect::to(uri!("/static/404.html"))))
    }
}

#[get("/get?<digicon_id>", rank = 3)]
pub async fn get_s3(
    user: UserIdGuard<Cookie>,
    _auth: Auth<DigiconContentReadable>,
    _type: Auth<DigiconStorageType<S3>>,
    hosting: &State<DigiconHosting>,
    digicon_id: DigiconGuard,
    aead: &State<AeadKey>,
    conn: DbConn,
) -> Result<DigiconFile, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    // Generate the trace string using UTC time, digicon ID, and the user ID.
    // These are the information that we don't want adversary to tamper with.
    let trace = format!(
        "{}:{}:{}",
        Utc::now().timestamp(),
        digicon.get_id(),
        &user.id.get_id(),
    );

    let mut resp = hosting
        .s3_client
        .get_object()
        .set_bucket(Some(hosting.s3_bucket.clone()))
        .set_key(Some(digicon.get_id().to_string()))
        .send()
        .await
        .into_flash(uri!("/"))?;

    let ctt_type = resp
        .content_type
        .and_then(|x| ContentType::parse_flexible(&x))
        // If there is no extension, we set content type to any
        .unwrap_or(ContentType::Any);

    let bytes = {
        let mut buf = BytesMut::with_capacity(MIN_CAPACITY).writer();

        while let Some(bytes) = resp.body.try_next().await.into_flash(uri!("/"))? {
            buf.write(&bytes).into_flash(uri!("/"))?;
        }
        buf.into_inner().freeze()
    };

    DigiconFile::from_response(bytes, ctt_type, aead, &trace).into_flash(uri!("/"))
}

#[get("/delete?<digicon_id>", rank = 1)]
pub async fn delete_release_asset(
    _auth: Auth<DigiconRemovable>,
    _type: Auth<DigiconStorageType<ReleaseAsset>>,
    digicon_id: DigiconGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;

    conn.run(move |c| digicon.delete(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/digicons")))
}

#[get("/delete?<digicon_id>", rank = 2)]
pub async fn delete_git_repo(
    _auth: Auth<DigiconRemovable>,
    _type: Auth<DigiconStorageType<GitRepo>>,
    hosting: &State<DigiconHosting>,
    digicon_id: DigiconGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;

    if digicon.get_storage_detail().is_some() {
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
            .bearer_auth(&hosting.gh_token)
            .json(&params)
            .send()
            .await
            .into_flash(uri!("/"))?
            .error_for_status()
            .into_flash(uri!("/"))?;
    }

    conn.run(move |c| digicon.delete(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/digicons")))
}

#[get("/delete?<digicon_id>", rank = 3)]
pub async fn delete_s3(
    _auth: Auth<DigiconRemovable>,
    _type: Auth<DigiconStorageType<S3>>,
    hosting: &State<DigiconHosting>,
    digicon_id: DigiconGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;

    hosting
        .s3_client
        .delete_object()
        .set_bucket(Some(hosting.s3_bucket.clone()))
        .set_key(Some(digicon.get_id().to_string()))
        .send()
        .await
        .into_flash(uri!("/"))?;

    conn.run(move |c| digicon.delete(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/digicons")))
}

#[post("/upload?<digicon_id>", data = "<file>", rank = 1)]
pub async fn upload_git_repo(
    _auth: Auth<DigiconWritable>,
    _type: Auth<DigiconStorageType<GitRepo>>,
    db: DbConn,
    digicon_id: DigiconGuard,
    hosting: &State<DigiconHosting>,
    file: Form<DigiconFile>,
) -> Result<Redirect, Status> {
    let digicon = digicon_id
        .to_digicon(&db)
        .await
        .map_err(|_| Status::new(502))?;

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
            "message": format!("create: {} - {}", digicon.get_name(), digicon.get_id()),
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
        .bearer_auth(&hosting.gh_token)
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
    Ok(Redirect::to(uri!("/digicons", digicon_page(id))))
}

#[post("/upload?<digicon_id>", data = "<file>", rank = 2)]
pub async fn upload_s3(
    _auth: Auth<DigiconWritable>,
    _type: Auth<DigiconStorageType<S3>>,
    db: DbConn,
    digicon_id: DigiconGuard,
    hosting: &State<DigiconHosting>,
    file: Form<DigiconFile>,
) -> Result<Redirect, Flash<Redirect>> {
    let digicon = digicon_id.to_digicon(&db).await.into_flash(uri!("/"))?;

    use md5::{Digest, Md5};

    // Calculate the hash of the file
    let mut hasher = Md5::new();
    hasher.update(&file.bytes);
    let hash = hasher.finalize();

    let id = digicon.get_id().to_string();

    hosting
        .s3_client
        .put_object()
        .set_bucket(Some(hosting.s3_bucket.clone()))
        .set_content_type(Some(file.ctt_type.to_string()))
        .set_content_md5(Some(base64::encode(hash)))
        .set_body(Some(file.bytes.clone().into()))
        .set_key(Some(digicon.get_id().to_string()))
        .send()
        .await
        .into_flash(uri!("/"))?;

    db.run(move |c| {
        digicon
            .set_storage_detail(Some(format!("{:x}", hash)))
            .update(c)
    })
    .await
    .into_flash(uri!("/"))?;

    Ok(Redirect::to(uri!("/digicons", digicon_page(id))))
}

#[get("/remove_digicon_mapping?<digicon_id>&<prod_id>")]
pub async fn remove_digicon_mapping(
    _digicon_guard: Auth<DigiconWritable>,
    _prod_guard: Auth<ProdWritable>,
    digicon_id: DigiconGuard,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    let digicon_cloned = digicon.clone();
    conn.run(move |c| {
        DigiconMappingFinder::new(c, None)
            .product(&prod.prod_id)
            .digicon(&digicon)
            .first()
            .map(|x| x.delete(c))
    })
    .await
    .into_flash(uri!("/"))?
    .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!(
        "/digicons",
        digicon_page(digicon_cloned.get_id())
    )))
}

#[get("/add_digicon_mapping?<digicon_id>&<prod_id>")]
pub async fn add_digicon_mapping(
    _digicon_guard: Auth<DigiconWritable>,
    _prod_guard: Auth<ProdWritable>,
    digicon_id: DigiconGuard,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    let digicon = digicon_id.to_digicon(&conn).await.into_flash(uri!("/"))?;
    let digicon_cloned = digicon.clone();
    conn.run(move |c| DigiconMapping::create(c, &digicon, &prod.prod_id))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!(
        "/digicons",
        digicon_page(digicon_cloned.get_id())
    )))
}
