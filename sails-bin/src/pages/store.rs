use crate::{
    infras::{guards::*, i18n::I18n},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{
    categories::*, enums::ProductStatus, error::SailsDbError, products::*, tags::*,
    users::UserInfo, Cmp,
};
use std::cmp::Ordering;

pub type ProductCard = (ProductInfo, Option<String>, LeafCategory, Vec<Tag>);

// Score the product and sort
pub fn cmp_product(this: &ProductCard, other: &ProductCard) -> Ordering {
    fn scoring(card: &ProductCard) -> usize {
        let mut score = 0;
        if card.1.is_some() {
            score += 2;
        }
        score += card.3.len();
        score
    }
    scoring(this).cmp(&scoring(other)).reverse()
}

pub fn find_first_image(fragment: &str) -> Option<String> {
    use select::{document::Document, predicate::Name};

    match Document::from_read(fragment.as_bytes()).ok().map(|doc| {
        doc.select(Name("img"))
            .next()
            .map(|x| x.attr("src").map(|s| s.to_string()))
    }) {
        Some(Some(s)) => s,
        _ => None,
    }
}

#[derive(Template)]
#[template(path = "store/home.html")]
pub struct StoreHomePage {
    i18n: I18n,
    pub entries: Vec<(LeafCategory, Vec<ProductCard>)>,
}

#[get("/")]
pub async fn home_page(i18n: I18n, conn: DbConn) -> Result<StoreHomePage, Flash<Redirect>> {
    let entries = conn
        .run(
            move |c| -> Result<Vec<(LeafCategory, Vec<ProductCard>)>, SailsDbError> {
                let categories = Categories::list_leaves::<Category>(c, None)?;
                categories
                    .into_iter()
                    .map(
                        |x| -> Result<(LeafCategory, Vec<ProductCard>), SailsDbError> {
                            let mut products = ProductFinder::new(c, None)
                                .status(ProductStatus::Verified, Cmp::Equal)
                                .category(&x)?
                                .search_info()?
                                .into_iter()
                                .filter_map(|x| {
                                    let image = find_first_image(x.get_description());
                                    let category = Categories::find_by_id(c, x.get_category_id())
                                        .and_then(Category::into_leaf)
                                        .ok()?;
                                    let tags = TagMappingFinder::new(c, None)
                                        .product(&x.to_id())
                                        .search_tag()
                                        .ok()?;
                                    Some(Ok((x, image, category, tags)))
                                })
                                // Reverse the prod order
                                .rev()
                                .collect::<Result<Vec<ProductCard>, SailsDbError>>()?;

                            products.sort_by(cmp_product);

                            Ok((x, products))
                        },
                    )
                    .collect()
            },
        )
        .await
        .into_flash(uri!("/"))?;
    Ok(StoreHomePage { entries, i18n })
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
    _guard: Auth<CanCreateProduct>,
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
