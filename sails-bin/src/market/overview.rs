use crate::{guards::BookGuard, DbConn, IntoFlash};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{categories::*, error::SailsDbError, products::*, Cmp};

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
    books: Vec<(ProductInfo, Option<String>, LeafCategory)>,
}

#[get("/explore?<table>", rank = 1)]
pub async fn explore_page(
    conn: DbConn,
    table: bool,
) -> Result<Result<ExplorePage, AllBooks>, Flash<Redirect>> {
    let books = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, Option<String>, LeafCategory)>, SailsDbError> {
                // TODO: We shall scope books by a father category
                // We only display allowed books
                let books_info = ProductFinder::new(c, None)
                    .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                    .search_info()?;

                books_info
                    .into_iter()
                    .map(|x| {
                        let image = find_first_image(x.get_description());
                        let category = Categories::find_by_id(c, x.get_category_id())
                            .and_then(Category::into_leaf)?;
                        Ok((x, image, category))
                    })
                    // Reverse the book order
                    .rev()
                    .collect()
            },
        )
        .await
        .into_flash(uri!("/"))?;
    if table {
        let books = books
            .into_iter()
            .map(|(prod, _, ctg)| (prod, ctg))
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
