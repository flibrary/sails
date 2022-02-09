use crate::{
    guards::*,
    market::{find_first_image, ProductCard},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{
    categories::*, enums::ProductStatus, error::SailsDbError, products::*, tags::*, Cmp,
};

#[derive(Template)]
#[template(path = "store/home.html")]
pub struct StoreHomePage {
    pub entries: Vec<(LeafCategory, Vec<ProductCard>)>,
}

#[get("/")]
pub async fn home_page(conn: DbConn) -> Result<StoreHomePage, Flash<Redirect>> {
    let entries = conn
        .run(
            move |c| -> Result<Vec<(LeafCategory, Vec<ProductCard>)>, SailsDbError> {
                let categories = Categories::list_leaves(
                    c,
                    Some(&Categories::find_by_name(c, "Store 在线商店")?),
                )?;
                categories
                    .into_iter()
                    .map(
                        |x| -> Result<(LeafCategory, Vec<ProductCard>), SailsDbError> {
                            let products = ProductFinder::new(c, None)
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
                                    if tags.iter().any(|x| x.get_id() == "store") {
                                        Some(Ok((x, image, category, tags)))
                                    } else {
                                        None
                                    }
                                })
                                // Reverse the book order
                                .rev()
                                .collect::<Result<Vec<ProductCard>, SailsDbError>>()?;
                            Ok((x, products))
                        },
                    )
                    .collect()
            },
        )
        .await
        .into_flash(uri!("/"))?;
    Ok(StoreHomePage { entries })
}
