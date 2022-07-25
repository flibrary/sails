use crate::{infras::guards::*, pages::store::*, sanitize_html, DbConn, IntoFlash};
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};
use sails_db::products::*;

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
    Ok(Redirect::to(uri!("/store", home_page)))
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
    _auth: Auth<CanCreateProduct>,
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
