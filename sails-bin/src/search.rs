use crate::{
    guards::BookGuard,
    market::{cmp_product, find_first_image, ProductCard},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{categories::*, error::SailsDbError, products::*, tags::*, Cmp};

#[derive(Template)]
#[template(path = "search/categories.html")]
pub struct CategoriesPage {
    categories: Option<Vec<Category>>,
    current_ctg: Option<Category>,
    parent_ctg: Option<Category>,
    products: Vec<ProductCard>,
}

// Browse all categories
#[get("/categories", rank = 2)]
pub async fn categories_all(conn: DbConn) -> Result<CategoriesPage, Flash<Redirect>> {
    let products = conn
        .run(move |c| -> Result<Vec<ProductCard>, SailsDbError> {
            // We only display allowed books
            let mut product_info = ProductFinder::new(c, None)
                .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
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

            product_info.sort_by(cmp_product);

            Ok(product_info)
        })
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
    let (category, parent_ctg, products) = conn
        .run(
            move |c| -> Result<(Category, Option<Category>, Vec<ProductCard>), SailsDbError> {
                let category = Categories::find_by_id(c, &category)?;
                let parent_category = category
                    .parent_id()
                    .map(|x| Categories::find_by_id(c, x))
                    .transpose()?;

                // We only display allowed books
                let mut product_info = ProductFinder::new(c, None)
                    .category(&category)?
                    .status(sails_db::enums::ProductStatus::Verified, Cmp::Equal)
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

                // Sort the products to make ones containing image appear on top.
                product_info.sort_by(cmp_product);

                Ok((category, parent_category, product_info))
            },
        )
        .await
        .into_flash(uri!("/"))?;

    Ok(CategoriesPage {
        current_ctg: Some(category.clone()),
        categories: if category.is_leaf() {
            None
        } else {
            Some(
                conn.run(move |c| category.subcategory(c))
                    .await
                    .into_flash(uri!("/"))?,
            )
        },
        products,
        parent_ctg,
    })
}
