use crate::{guards::BookGuard, DbConn, IntoFlash};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{categories::*, error::SailsDbError, products::*, Cmp};

#[derive(Template)]
#[template(path = "market/categories.html")]
pub struct CategoriesPage {
    categories: Vec<Category>,
}

// If there is no category specified, we simply go for the top categories
#[get("/categories", rank = 2)]
pub async fn categories_all(conn: DbConn) -> Result<CategoriesPage, Flash<Redirect>> {
    conn.run(move |c| Categories::list_top(c))
        .await
        .into_flash(uri!("/"))
        .map(|v| CategoriesPage { categories: v })
}

// Category browsing
#[get("/categories?<category>", rank = 1)]
pub async fn categories(
    conn: DbConn,
    category: String,
) -> Result<Result<CategoriesPage, Redirect>, Flash<Redirect>> {
    // There is a specified category name
    let category_cloned = category.clone();
    let category = conn
        .run(move |c| Categories::find_by_id(c, &category))
        .await
        .into_flash(uri!("/"))?;

    // The category is a leaf, meaning that we then have to search for books related to that
    if category.is_leaf() {
        Ok(Err(Redirect::to(uri!(
            "/market",
            explore_page_ctg(category_cloned)
        ))))
    } else {
        // The category is not a leaf, continuing down the path
        Ok(Ok(CategoriesPage {
            categories: conn
                .run(move |c| category.subcategory(c))
                .await
                .into_flash(uri!("/"))?,
        }))
    }
}

#[derive(Template)]
#[template(path = "market/all_books.html")]
pub struct AllBooks {
    // By using Option<Category>, we ensure thatthere will be no panick even if category doesn't exist
    books: Vec<(ProductInfo, Option<Category>)>,
    ctg: Option<String>,
}

// List all products
#[get("/all_books?<category>", rank = 1)]
pub async fn all_books_category(
    conn: DbConn,
    category: String,
) -> Result<AllBooks, Flash<Redirect>> {
    let ctg = category.clone();
    Ok(AllBooks {
        books: conn
            .run(
                move |c| -> Result<Vec<(ProductInfo, Option<Category>)>, SailsDbError> {
                    let ctg = Categories::find_by_id(c, &category).and_then(Category::into_leaf)?;
                    // We only display allowed books
                    let books_info = ProductFinder::new(c, None)
                        .category(&ctg)
                        .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                        .search_info()?;

                    books_info
                        .into_iter()
                        .map(|x| {
                            let ctg = Categories::find_by_id(c, x.get_category_id()).ok();
                            Ok((x, ctg))
                        })
                        // Reverse the book order
                        .rev()
                        .collect()
                },
            )
            .await
            // We have to redirect errors all the way back to the index page, otherwise a deadlock is formed
            .into_flash(uri!("/"))?,
        ctg: Some(ctg),
    })
}

#[get("/all_books", rank = 2)]
pub async fn all_books(conn: DbConn) -> Result<AllBooks, Flash<Redirect>> {
    Ok(AllBooks {
        books: conn
            .run(
                move |c| -> Result<Vec<(ProductInfo, Option<Category>)>, SailsDbError> {
                    // We only display allowed books
                    let books_info = ProductFinder::new(c, None)
                        .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                        .search_info()?;

                    books_info
                        .into_iter()
                        .map(|x| {
                            let ctg = Categories::find_by_id(c, x.get_category_id()).ok();
                            Ok((x, ctg))
                        })
                        // Reverse the book order
                        .rev()
                        .collect()
                },
            )
            .await
            .into_flash(uri!("/"))?,
        ctg: None,
    })
}

#[derive(Template)]
#[template(path = "market/explore.html")]
pub struct ExplorePage {
    // By using Option<Category>, we ensure thatthere will be no panick even if category doesn't exist
    books: Vec<(ProductInfo, Option<String>)>,
    ctg: Option<String>,
}

fn find_first_image(fragment: &str) -> Option<String> {
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

#[get("/explore?<category>", rank = 1)]
pub async fn explore_page_ctg(
    conn: DbConn,
    category: String,
) -> Result<ExplorePage, Flash<Redirect>> {
    let ctg = category.clone();
    Ok(ExplorePage {
        books: conn
            .run(
                move |c| -> Result<Vec<(ProductInfo, Option<String>)>, SailsDbError> {
                    let ctg = Categories::find_by_id(c, &category).and_then(Category::into_leaf)?;
                    // We only display allowed books
                    let books_info = ProductFinder::new(c, None)
                        .category(&ctg)
                        .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                        .search_info()?;

                    books_info
                        .into_iter()
                        .map(|x| {
                            let image = find_first_image(x.get_description());
                            Ok((x, image))
                        })
                        // Reverse the book order
                        .rev()
                        .collect()
                },
            )
            .await
            .into_flash(uri!("/"))?,
        ctg: Some(ctg),
    })
}

#[get("/explore", rank = 2)]
pub async fn explore_page(conn: DbConn) -> Result<ExplorePage, Flash<Redirect>> {
    Ok(ExplorePage {
        books: conn
            .run(
                move |c| -> Result<Vec<(ProductInfo, Option<String>)>, SailsDbError> {
                    // We only display allowed books
                    let books_info = ProductFinder::new(c, None)
                        .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                        .search_info()?;

                    books_info
                        .into_iter()
                        .map(|x| {
                            let image = find_first_image(x.get_description());
                            Ok((x, image))
                        })
                        // Reverse the book order
                        .rev()
                        .collect()
                },
            )
            .await
            .into_flash(uri!("/"))?,
        ctg: None,
    })
}

#[get("/")]
pub async fn market() -> Redirect {
    Redirect::to(uri!("/market", explore_page))
}
