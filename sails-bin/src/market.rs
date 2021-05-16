use rocket::{
    form::Form,
    http::{Cookie, CookieJar},
    request::FlashMessage,
    response::{Flash, Redirect},
};
use rocket_contrib::templates::Template;
use sails_db::{
    categories::*,
    products::{Product, ProductFinder, Products},
    users::{User, Users},
};
use serde::Serialize;
use serde_json::json;
use std::num::NonZeroI64;

use crate::{Context, DbConn};

const NAMESPACE: &'static str = "/market";

// Form used for creating new books
#[derive(FromForm)]
pub struct BookInfo {
    category: String,
    prodname: String,
    price: NonZeroI64,
    description: String,
}

#[post("/create_book", data = "<info>")]
pub async fn create_book(
    jar: Option<&CookieJar<'_>>,
    info: Form<BookInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    if let Some(Some(uid)) = jar.map(|j| j.get_private("uid")) {
        match conn
            .run(move |c| {
                Products::create_product(
                    c,
                    uid.value(),
                    &info.category,
                    &info.prodname,
                    info.price,
                    &info.description,
                )
            })
            .await
        {
            Ok(product_id) => {
                // TODO: Change this to product page
                Ok(Redirect::to(uri!("/market", book_page: product_id)))
            }
            Err(e) => Err(Flash::error(
                Redirect::to(uri!("/market", post_book)),
                e.to_string(),
            )),
        }
    } else {
        Err(Flash::error(Redirect::to("/user"), "login to post books"))
    }
}

#[get("/post_book")]
pub async fn post_book(conn: DbConn, flash: Option<FlashMessage<'_>>) -> Template {
    match conn.run(move |c| Categories::list_leaves(c)).await {
        Ok(v) => {
            let mut cx = Context::new(v);
            cx.with_flash(flash);
            return Template::render("post_book", cx);
        }
        // Other errors
        Err(e) => return Template::render("post_book", Context::msg(json!({}), e.to_string())),
    }
}

#[get("/product?<book_id>")]
pub async fn book_page(
    jar: Option<&CookieJar<'_>>,
    conn: DbConn,
    book_id: String,
) -> Result<Template, Flash<Redirect>> {
    // A temporary struct for book page content
    #[derive(Serialize)]
    struct BookPage {
        book: Product,
        categories: Vec<Category>,
        seller: User,
        signedin: bool,
        is_seller: bool,
    }

    match conn
        .run(move |c| ProductFinder::new(c, None).id(&book_id).search())
        .await
    {
        Ok(v) if v.is_empty() => Err(Flash::error(
            Redirect::to(NAMESPACE),
            "no book found with the given book ID",
        )),
        Ok(mut v) => {
            let v = v.pop().unwrap();
            let (signedin, is_seller) = match jar.map(|j| j.get_private("uid")) {
                Some(Some(id)) => {
                    if v.seller_id() == id.value() {
                        // The visitor is the seller
                        (true, true)
                    } else {
                        // The visitor is not the seller
                        (true, false)
                    }
                }
                // Either we don't have cookie jar or the cookie jar doesn't contain UID
                _ => (false, false),
            };
            let seller_id = v.seller_id().to_string();
            let seller = conn
                .run(move |c| Users::find_by_id(c, &seller_id))
                .await
                .unwrap();
            let categories = conn.run(move |c| Categories::list_leaves(c)).await.unwrap();
            // There should be only one book found because we are using uuid
            Ok(Template::render(
                "product",
                Context::new(BookPage {
                    book: v,
                    seller: seller,
                    categories,
                    signedin,
                    is_seller,
                }),
            ))
        }
        Err(e) => Err(Flash::error(Redirect::to(NAMESPACE), e.to_string())),
    }
}

#[get("/categories?<name>")]
pub async fn categories(conn: DbConn, name: Option<String>) -> Result<Template, Redirect> {
    if let Some(ctg) = name {
        // There is a specified category name
        let ctg_cloned = ctg.clone();
        match conn
            .run(move |c| Categories::find_by_id(c, &ctg_cloned))
            .await
        {
            Ok(category) => {
                // The category is a leaf, meaning that we then have to search for books related to that
                if category.is_leaf() {
                    Err(Redirect::to(uri!("/market", all_products: Some(ctg))))
                } else {
                    // The category is not a leaf, continuing down the path
                    Ok(Template::render(
                        "by_categories",
                        Context::new(
                            conn.run(move |c| Categories::subcategory(c, &ctg))
                                .await
                                .unwrap(),
                        ),
                    ))
                }
            }
            // Other error encountered
            Err(e) => Ok(Template::render(
                "by_categories",
                Context::msg(json!({}), e.to_string()),
            )),
        }
    } else {
        // If there is no category specified, we simply go for the top categories
        match conn.run(move |c| Categories::list_top(c)).await {
            // There is no top category, this is possible if the categories are looped together. (That is a bad taste BTW)
            Ok(v) if v.is_empty() => Ok(Template::render(
                "by_categories",
                Context::msg(json!({}), "There is no top category"),
            )),
            Ok(v) => Ok(Template::render("by_categories", Context::new(v))),
            // Other errors
            Err(e) => Ok(Template::render(
                "by_categories",
                Context::msg(json!({}), e.to_string()),
            )),
        }
    }
}

#[get("/all_products?<category>")]
pub async fn all_products(
    conn: DbConn,
    category: Option<String>,
    flash: Option<FlashMessage<'_>>,
) -> Template {
    let mut cx = if let Some(name) = category {
        Context::new(
            conn.run(move |c| ProductFinder::new(c, None).category(&name).search())
                .await
                .unwrap(),
        )
    } else {
        Context::new(conn.run(move |c| Products::list(c)).await.unwrap())
    };
    cx.with_flash(flash);
    // Go for all products by default
    Template::render("all_products", cx)
}

#[get("/")]
pub async fn market() -> Redirect {
    Redirect::to(uri!("/market", all_products: _))
}
