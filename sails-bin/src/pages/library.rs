use super::store::{cmp_product, find_first_image};
use crate::{
    infras::{guards::*, i18n::I18n},
    DbConn, IntoFlash,
};
use askama::Template;
use rocket::response::{Flash, Redirect};
use sails_db::{
    categories::*,
    digicons::{Digicon, DigiconMappingFinder, Digicons},
    error::SailsDbError,
    products::*,
    tags::*,
};

pub type ProductCard = (ProductInfo, Option<String>, LeafCategory, Vec<Tag>);

#[derive(Template)]
#[template(path = "library/home.html")]
pub struct LibHomePage {
    i18n: I18n,
    pub prods: Vec<ProductCard>,
    pub digicons_owned: Vec<Digicon>,
}

#[get("/")]
pub async fn home_page(
    i18n: I18n,
    conn: DbConn,
    user: UserIdGuard<Cookie>,
) -> Result<LibHomePage, Flash<Redirect>> {
    let (digicons_owned, prods) = conn
        .run(
            move |c| -> Result<(Vec<Digicon>, Vec<ProductCard>), SailsDbError> {
                let mut products = ProductFinder::readable_digital_products_info(c, &user.id)?
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

                let digicons_owned = Digicons::list_all_content_readable(c, &user.id)?;

                Ok((digicons_owned, products))
            },
        )
        .await
        .into_flash(uri!("/"))?;
    Ok(LibHomePage {
        digicons_owned,
        prods,
        i18n,
    })
}

#[derive(Template)]
#[template(path = "library/prod.html")]
pub struct LibProdPage {
    i18n: I18n,
    digicons: Vec<Digicon>,
    prod: ProductInfo,
}

#[get("/prod_info?<prod_id>")]
pub async fn prod_page(
    i18n: I18n,
    conn: DbConn,
    prod_id: ProdGuard,
    _user: UserIdGuard<Cookie>,
) -> Result<LibProdPage, Flash<Redirect>> {
    let prod = prod_id.to_info(&conn).await.into_flash(uri!("/"))?;
    let prod_info = prod.prod_info.clone();
    let digicons = conn
        .run(move |c| -> Result<Vec<Digicon>, SailsDbError> {
            DigiconMappingFinder::new(c, None)
                .product(&prod.prod_id)
                .search_digicon()
        })
        .await
        .into_flash(uri!("/"))?;
    Ok(LibProdPage {
        digicons,
        i18n,
        prod: prod_info,
    })
}
