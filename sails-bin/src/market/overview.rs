use crate::{guards::BookGuard, DbConn, IntoFlash};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{categories::*, error::SailsDbError, products::*, tags::*, Cmp};
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
#[template(path = "market/all_books.html")]
pub struct AllBooks {
    books: Vec<(ProductInfo, LeafCategory)>,
}

#[derive(Template)]
#[template(path = "market/explore.html")]
pub struct ExplorePage {
    books: Vec<ProductCard>,
}

#[get("/explore?<table>", rank = 1)]
pub async fn explore_page(
    conn: DbConn,
    table: bool,
) -> Result<Result<ExplorePage, AllBooks>, Flash<Redirect>> {
    let books = conn
        .run(move |c| -> Result<Vec<ProductCard>, SailsDbError> {
            // TODO: We shall scope books by a father category
            // We only display allowed books
            let mut book_info = ProductFinder::new(c, None)
                .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                .category(&Categories::find_by_name(c, "书本市场")?)?
                .search_info()?
                .into_iter()
                .map(|x| {
                    let image = find_first_image(x.get_description());
                    let category = Categories::find_by_id(c, x.get_category_id())
                        .and_then(Category::into_leaf)?;
                    let tags = TagMappingFinder::new(c, None)
                        .product(&x.to_id())
                        .search_tag()?;
                    Ok((x, image, category, tags))
                })
                // Reverse the book order
                .rev()
                .collect::<Result<Vec<ProductCard>, SailsDbError>>()?;

            book_info.sort_by(cmp_product);
            Ok(book_info)
        })
        .await
        .into_flash(uri!("/"))?;
    if table {
        let books = books
            .into_iter()
            .map(|(prod, _, ctg, _)| (prod, ctg))
            .collect();
        Ok(Err(AllBooks { books }))
    } else {
        Ok(Ok(ExplorePage { books }))
    }
}

#[get("/")]
pub async fn market() -> Redirect {
    Redirect::to(uri!("/market", explore_page(false)))
}
