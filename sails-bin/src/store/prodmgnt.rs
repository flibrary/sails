use crate::{guards::*, sanitize_html, DbConn, IntoFlash};
use askama::Template;
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};
use rocket_i18n::I18n;
use sails_db::{categories::*, products::*, tags::*, users::*};

// Delete can happen if and only if the user is authorized and the product is specified
#[get("/delete?<prod_id>")]
pub async fn delete_prod(
    _auth: Auth<ProdRemovable>,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    conn.run(move |c| prod.prod_id.delete(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/store", super::home_page)))
}

// Handle prod creation or update
// If the product is unspecified, then we are in creating mode, else we are updating
// For either creating a prod or updating a prod, the user must be signed in.
// For updating a prod, the user must additionally be authorized
// Notice that we have to then redirect users on post_prod page to user portal if they are not logged in

// Update the prod, this is more specific than creation, meaning that it should be routed first
#[post("/cow_prod?<prod_id>", data = "<info>", rank = 1)]
pub async fn update_prod(
    prod_id: ProdGuard,
    _auth: Auth<ProdWritable>,
    mut info: Form<IncompleteProductOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let prod = prod_id.to_id(&conn).await.into_flash(uri!("/"))?;
    info.description = sanitize_html(&info.description);
    // The user is the seller, he/she is authorized
    conn.run(move |c| prod.prod_id.update_owned(c, info.into_inner().verify(c)?))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/store", prod_page_owned(prod_id))))
}

// User is logged in, creating the prod.
#[post("/cow_prod", data = "<info>", rank = 2)]
pub async fn create_prod(
    user: UserIdGuard<Cookie>,
    _auth: Auth<StoreModifiable>,
    mut info: Form<IncompleteProductOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    info.description = sanitize_html(&info.description);
    let product_id = conn
        .run(move |c| info.create(c, &user.id))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!(
        "/store",
        prod_page_owned(product_id.get_id())
    )))
}

#[derive(Template)]
#[template(path = "store/update_prod.html")]
pub struct UpdateProd {
    i18n: I18n,
    prod: ProductInfo,
    categories: Vec<LeafCategory>,
}

#[derive(Template)]
#[template(path = "store/post_prod.html")]
pub struct PostProd {
    i18n: I18n,
    categories: Vec<LeafCategory>,
}

// If there is a prod specified, we then use the default value of that specified prod for update
#[get("/post_prod?<prod_id>", rank = 1)]
pub async fn update_prod_page(
    i18n: I18n,
    conn: DbConn,
    _auth: Auth<ProdWritable>,
    prod_id: ProdGuard,
) -> Result<UpdateProd, Flash<Redirect>> {
    let prod = prod_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(UpdateProd {
        i18n,
        // If there is no leaves, user cannot create any prods, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: conn
            .run(move |c| Categories::list_leaves::<LeafCategory>(c, None))
            .await
            .into_flash(uri!("/"))?,
        prod: prod.prod_info,
    })
}

// No prod specified
#[get("/post_prod", rank = 2)]
pub async fn post_prod_page(
    i18n: I18n,
    conn: DbConn,
    _guard: Auth<StoreModifiable>,
    _user: UserIdGuard<Cookie>,
) -> Result<PostProd, Flash<Redirect>> {
    Ok(PostProd {
        i18n,
        // If there is no leaves, user cannot create any prods, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: conn
            .run(move |c| Categories::list_leaves::<LeafCategory>(c, None))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[get("/post_prod", rank = 3)]
pub async fn post_prod_error_page() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/")),
        "please check if you have logged in and authorized to update/create",
    )
}

#[derive(Template)]
#[template(path = "store/prod_info_owned.html")]
pub struct ProdPageOwned {
    i18n: I18n,
    prod: ProductInfo,
    category: Option<LeafCategory>,
    seller: UserInfo,
    tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "store/prod_info_user.html")]
pub struct ProdPageUser {
    i18n: I18n,
    prod: ProductInfo,
    category: Option<LeafCategory>,
    seller: UserInfo,
    tags: Vec<Tag>,
}

#[derive(Template)]
#[template(path = "store/prod_info_guest.html")]
pub struct ProdPageGuest {
    i18n: I18n,
    prod: ProductInfo,
    category: Option<LeafCategory>,
    tags: Vec<Tag>,
}

// If the seller is the user, buttons like update and delete are displayed
#[get("/prod_info?<prod_id>", rank = 1)]
pub async fn prod_page_owned(
    i18n: I18n,
    prod_id: ProdGuard,
    conn: DbConn,
    _auth: Auth<ProdWritable>,
) -> Result<ProdPageOwned, Flash<Redirect>> {
    let prod = prod_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(ProdPageOwned {
        i18n,
        prod: prod.prod_info,
        tags: prod.tags,
        category: prod
            .category
            .map(|x| x.into_leaf().into_flash(uri!("/")))
            .transpose()?,
        seller: prod.seller_info,
    })
}

// If the user is signed in but not authorized, prod information and seller information will be displayed
#[get("/prod_info?<prod_id>", rank = 2)]
pub async fn prod_page_user(
    i18n: I18n,
    prod_id: ProdGuard,
    conn: DbConn,
    _auth: Auth<ProdReadable>,
) -> Result<ProdPageUser, Flash<Redirect>> {
    let prod = prod_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(ProdPageUser {
        i18n,
        prod: prod.prod_info,
        tags: prod.tags,
        category: prod
            .category
            .map(|x| x.into_leaf().into_flash(uri!("/")))
            .transpose()?,
        seller: prod.seller_info,
    })
}

// If the user is not signed in, only prod information will be displayed
#[get("/prod_info?<prod_id>", rank = 3)]
pub async fn prod_page_guest(
    i18n: I18n,
    prod_id: ProdGuard,
    conn: DbConn,
) -> Result<ProdPageGuest, Flash<Redirect>> {
    let prod = prod_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(ProdPageGuest {
        i18n,
        prod: prod.prod_info,
        tags: prod.tags,
        category: prod
            .category
            .map(|x| x.into_leaf().into_flash(uri!("/")))
            .transpose()?,
    })
}

// If the prod is not specified, error id returned
#[get("/prod_info", rank = 4)]
pub async fn prod_page_error() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/")),
        "no prod found with the given product ID",
    )
}
