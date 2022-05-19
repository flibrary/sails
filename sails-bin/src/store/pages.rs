use crate::{guards::*, i18n::I18n, DbConn, IntoFlash};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{
    categories::*, enums::ProductStatus, error::SailsDbError, products::*, tags::*, Cmp,
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
                                    Some(Ok((x, image, category, tags)))
                                })
                                // Reverse the prod order
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
    Ok(StoreHomePage { entries, i18n })
}
