use crate::{
    infras::{guards::ProdGuard, i18n::I18n},
    pages::store::{cmp_product, find_first_image, ProductCard},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{categories::*, error::SailsDbError, products::*, tags::*, Cmp};

#[derive(Template)]
#[template(path = "search/categories.html")]
pub struct CategoriesPage {
    i18n: I18n,
    categories: Option<Vec<Category>>,
    current_ctg: Option<Category>,
    parent_ctgs: Vec<Category>,
    products: Vec<ProductCard>,
}

// Browse all categories
#[get("/categories", rank = 2)]
pub async fn categories_all(i18n: I18n, conn: DbConn) -> Result<CategoriesPage, Flash<Redirect>> {
    let products = conn
        .run(move |c| -> Result<Vec<ProductCard>, SailsDbError> {
            // We only display allowed prods
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
                // Reverse the prod order
                .rev()
                .collect::<Result<Vec<ProductCard>, SailsDbError>>()?;

            product_info.sort_by(cmp_product);

            Ok(product_info)
        })
        .await
        .into_flash(uri!("/"))?;

    Ok(CategoriesPage {
        i18n,
        current_ctg: None,
        // We are required to list all
        categories: Some(
            conn.run(move |c| Categories::list_top(c))
                .await
                .into_flash(uri!("/"))?,
        ),
        products,
        parent_ctgs: Vec::new(),
    })
}

// Category browsing
#[get("/categories?<category>", rank = 1)]
pub async fn categories(
    i18n: I18n,
    conn: DbConn,
    category: String,
) -> Result<CategoriesPage, Flash<Redirect>> {
    // We didn't use map for that we want to throw out errors.
    let (category, parent_ctgs, products) = conn
        .run(
            move |c| -> Result<(Category, Vec<Category>, Vec<ProductCard>), SailsDbError> {
                let category = Categories::find_by_id(c, &category)?;

                let mut parent_categories = Vec::new();
                // The parent ID of the current category
                let mut current_parent = category.parent_id();
                while let Some(parent_ctg) = current_parent
                    .map(|x| Categories::find_by_id(c, x))
                    .transpose()?
                {
                    parent_categories.insert(0, parent_ctg);
                    // Change the "current parent" to the parent of the "current parent"
                    current_parent = parent_categories[0].parent_id();
                }

                // We only display allowed prods
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
                    // Reverse the prod order
                    .rev()
                    .collect::<Result<Vec<ProductCard>, SailsDbError>>()?;

                // Sort the products to make ones containing image appear on top.
                product_info.sort_by(cmp_product);

                Ok((category, parent_categories, product_info))
            },
        )
        .await
        .into_flash(uri!("/"))?;

    Ok(CategoriesPage {
        i18n,
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
        parent_ctgs,
    })
}
