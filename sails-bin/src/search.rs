use crate::{guards::BookGuard, market::find_first_image, DbConn, IntoFlash};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{categories::*, error::SailsDbError, products::*, Cmp};

#[derive(Template)]
#[template(path = "search/categories.html")]
pub struct CategoriesPage {
    categories: Option<Vec<Category>>,
    current_ctg: Option<Category>,
    parent_ctg: Option<Category>,
    products: Vec<(ProductInfo, Option<String>, LeafCategory)>,
}

// Browse all categories
#[get("/categories", rank = 2)]
pub async fn categories_all(conn: DbConn) -> Result<CategoriesPage, Flash<Redirect>> {
    let products = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, Option<String>, LeafCategory)>, SailsDbError> {
                // We only display allowed books
                let products_info = ProductFinder::new(c, None)
                    .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                    .search_info()?;

                products_info
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

    Ok(CategoriesPage {
        current_ctg: None,
        // We are required to list all
        categories: Some(
            conn.run(move |c| Categories::list_top(c))
                .await
                .into_flash(uri!("/"))?,
        ),
        products,
        parent_ctg: None,
    })
}

// Category browsing
#[get("/categories?<category>", rank = 1)]
pub async fn categories(conn: DbConn, category: String) -> Result<CategoriesPage, Flash<Redirect>> {
    // We didn't use map for that we want to throw out errors.
    let category = conn
        .run(move |c| Categories::find_by_id(c, &category))
        .await
        .into_flash(uri!("/"))?;
    let category_cloned = category.clone();
    let category_cloned_2 = category.clone();
    let parent_ctg = conn
        .run(move |c| {
            category_cloned_2
                .parent_id()
                .map(|x| Categories::find_by_id(c, x))
        })
        .await
        .transpose()
        .into_flash(uri!("/"))?;

    let products = conn
        .run(
            move |c| -> Result<Vec<(ProductInfo, Option<String>, LeafCategory)>, SailsDbError> {
                // We only display allowed books
                let products_info = ProductFinder::new(c, None)
                    .category(&category)?
                    .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
                    .search_info()?;

                products_info
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

    Ok(CategoriesPage {
        current_ctg: Some(category_cloned.clone()),
        categories: if category_cloned.is_leaf() {
            None
        } else {
            Some(
                conn.run(move |c| category_cloned.subcategory(c))
                    .await
                    .into_flash(uri!("/"))?,
            )
        },
        products,
        parent_ctg,
    })
}
