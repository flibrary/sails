use rocket::{
    form::Form,
    outcome::{try_outcome, Outcome},
    request::{FlashMessage, FromRequest},
    response::{Flash, Redirect},
};
use rocket_contrib::templates::Template;
use sails_db::{
    categories::*,
    error::SailsDbError,
    products::{Product, ProductFinder, Products, UpdateProduct},
    users::{User, Users},
};
use serde::Serialize;

use crate::{user::UserWrap, wrap_op, Context, DbConn};

const NAMESPACE: &str = "/market";

// This request guard gets us the product specified using `book_id` parameter if there is one
// If there is a valid `book_id` specified, then the product and user related will be retrieved.
// If the user is signed in, the user information will be retrieved. And a bool value indicate if the user the user is authorized to mutate the product.
pub struct MarketInfo {
    pub product: Option<(Product, User)>,
    pub user: Option<(User, bool)>,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for MarketInfo {
    type Error = ();

    // This from_request will never fail
    async fn from_request(
        request: &'r rocket::Request<'_>,
    ) -> rocket::request::Outcome<Self, Self::Error> {
        let db = try_outcome!(request.guard::<DbConn>().await);
        let user = request.guard::<UserWrap>().await.succeeded();
        let product_id = request
            .query_value::<String>("book_id")
            .and_then(|x| x.ok());

        let product = if let Some(product_id) = product_id {
            db.run(move |c| -> Result<(Product, User), SailsDbError> {
                let p = ProductFinder::new(c, None)
                    .id(&product_id)
                    .search()
                    // WARN: Ok() doesn't imply that there is at least one element
                    .and_then(|mut p| {
                        if !p.is_empty() {
                            Ok(p.remove(0))
                        } else {
                            Err(SailsDbError::ProductNotFound)
                        }
                    })?;
                // This shall never fail
                let u = Users::find_by_id(c, p.get_seller_id())?;
                Ok((p, u))
            })
            .await
            .ok()
        } else {
            None
        };
        let user = if let Some((_, ref seller)) = product {
            user.map(|u| {
                let user_inner = u.inner();
                let authorized = seller.get_id() == user_inner.get_id();
                (user_inner, authorized)
            })
        } else {
            user.map(|u| (u.inner(), false))
        };
        Outcome::Success(MarketInfo { product, user })
    }
}

// Delete can happen if and only if the user is authorized and the product is specified
#[get("/delete")]
pub async fn delete_book(info: MarketInfo, conn: DbConn) -> Result<Redirect, Flash<Redirect>> {
    match info {
        MarketInfo {
            product: Some((product, _)),
            user: Some((_, authorized)),
        } if authorized => {
            wrap_op(
                conn.run(move |c| Products::delete_by_id(c, product.get_id()))
                    .await,
                NAMESPACE,
            )?;
            Ok(Redirect::to(NAMESPACE))
        }
        _ => Err(Flash::error(
            Redirect::to(NAMESPACE),
            "not authorized or book ID invalid",
        )),
    }
}

// Form used for creating new/updating books
// The reason why we don't directly use the UpdateProduct struct is that we have to use the same form for both update and creation. While updates allows left out fields, creation doesn't. And we use auto-completion on the frontend to comply with this design choice.
#[derive(FromForm)]
pub struct BookInfo {
    category: String,
    prodname: String,
    price: i64,
    description: String,
}

impl From<BookInfo> for UpdateProduct {
    fn from(x: BookInfo) -> Self {
        UpdateProduct {
            category: Some(x.category),
            prodname: Some(x.prodname),
            price: Some(x.price),
            description: Some(x.description),
        }
    }
}

// Handle book creation or update
// If the product is unspecified, then we are in creating mode, else we are updating
// For either creating a book or updating a book, the user must be signed in.
// For updating a book, the user must additionally be authorized
#[post("/update_book", data = "<info>")]
pub async fn update_book(
    mktinfo: MarketInfo,
    info: Form<BookInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    match mktinfo {
        // Product is specified and the user is logged in and authorized
        MarketInfo {
            product: Some((mut product, _)),
            user: Some((_, authorized)),
        } if authorized => {
            let product_id = product.get_id().to_string();
            // The user is the seller, he/she is authorized
            product.update(info.into_inner().into());
            wrap_op(
                conn.run(move |c| Products::update(c, product)).await,
                uri!("/market", post_book),
            )?;
            Ok(Redirect::to(format!(
                "/market/product?book_id={}",
                product_id,
            )))
        }
        // Product is specified and user is logged in but NOT authorized
        MarketInfo {
            product: Some(_),
            user: Some((_, authorized)),
        } if !authorized => {
            // Unauthorized update
            Err(Flash::error(
                Redirect::to("/market"),
                "you are not authorized to update the book posted",
            ))
        }
        // Product is not specified, but user is logged in. We are creating products
        MarketInfo {
            product: None,
            user: Some((user, _)),
        } => {
            let product_id = wrap_op(
                conn.run(move |c| {
                    Products::create(
                        c,
                        user.get_id(),
                        &info.category,
                        &info.prodname,
                        info.price,
                        &info.description,
                    )
                })
                .await,
                uri!("/market", post_book),
            )?;
            Ok(Redirect::to(format!(
                "/market/product?book_id={}",
                product_id
            )))
        }
        // User not signed in, we cannot do anything
        _ => Err(Flash::error(
            Redirect::to("/market"),
            "you must sign in to update or post books",
        )),
    }
}

// post_book page
// If there is a book specified, we then use the default value of that specified book for update
#[get("/post_book")]
pub async fn post_book(conn: DbConn, info: MarketInfo) -> Result<Template, Flash<Redirect>> {
    #[derive(Serialize)]
    struct PostBook {
        book: Option<Product>,
        categories: Vec<Category>,
    }

    Ok(Template::render(
        if info.product.is_some() {
            "update_book"
        } else {
            "post_book"
        },
        Context::from_content(PostBook {
            // If there is no leaves, user cannot create any books, a message should be displayed inside the template
            // TODO: categories should only be fetched once
            categories: wrap_op(
                conn.run(move |c| Categories::list_leaves(c)).await,
                uri!("/market", all_products: _),
            )?,
            book: info.product.map(|info| info.0),
        }),
    ))
}

// Book information display page
// If the book is not specified, error id returned
// If the user is not signed in, only book information will be displayed
// If the user is signed in but not authorized, book information and seller information will be displayed
// If the seller is the user, buttons like update and delete are displayed
#[get("/product")]
pub async fn book_page(info: MarketInfo) -> Result<Template, Flash<Redirect>> {
    // A temporary struct for book page content
    #[derive(Serialize)]
    struct BookPage {
        book: Product,
        seller: Option<User>,
    }

    match info {
        // If the book is not specified, error is returned
        MarketInfo {
            product: None,
            user: _,
        } => Err(Flash::error(
            Redirect::to(NAMESPACE),
            "no book found with the given book ID",
        )),
        // If the user is not signed in, only book information will be displayed
        MarketInfo {
            product: Some((product, _)),
            user: None,
        } => Ok(Template::render(
            "product",
            Context::from_content(BookPage {
                book: product,
                seller: None,
            }),
        )),
        // If the user is signed in but not authorized, book information and seller information will be displayed
        MarketInfo {
            product: Some((product, seller)),
            user: Some((_, authorized)),
        } if !authorized => Ok(Template::render(
            "product",
            Context::from_content(BookPage {
                book: product,
                seller: Some(seller),
            }),
        )),
        MarketInfo {
            product: Some((product, _)),
            user: Some((_, authorized)),
        } if authorized => Ok(Template::render(
            "product_owned",
            Context::from_content(BookPage {
                book: product,
                // We don't need seller info
                seller: None,
            }),
        )),
        _ => unreachable!(),
    }
}

// Category browsing
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
                        Context::from_content(
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
                Context::err(e.to_string()),
            )),
        }
    } else {
        // If there is no category specified, we simply go for the top categories
        match conn.run(move |c| Categories::list_top(c)).await {
            // There is no top category, this is possible if the categories are looped together. (That is a bad taste BTW)
            Ok(v) if v.is_empty() => Ok(Template::render(
                "by_categories",
                Context::err("There is no top category"),
            )),
            Ok(v) => Ok(Template::render("by_categories", Context::from_content(v))),
            // Other errors
            Err(e) => Ok(Template::render(
                "by_categories",
                Context::err(e.to_string()),
            )),
        }
    }
}

// List all products
#[get("/all_products?<category>")]
pub async fn all_products(
    conn: DbConn,
    category: Option<String>,
    flash: Option<FlashMessage<'_>>,
) -> Result<Template, Flash<Redirect>> {
    let cx = Context::new(
        if let Some(name) = category {
            wrap_op(
                conn.run(move |c| ProductFinder::new(c, None).category(&name).search())
                    .await,
                "/",
            )?
        } else {
            // Go for all products by default
            wrap_op(conn.run(move |c| Products::list(c)).await, "/")?
        },
        flash,
    );
    Ok(Template::render("all_products", cx))
}

#[get("/")]
pub async fn market() -> Redirect {
    Redirect::to(uri!("/market", all_products: _))
}
