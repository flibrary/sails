use askama::Template;
use rocket::{
    form::Form,
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{
    categories::*,
    products::{Product, ProductFinder, Products, UpdateProduct},
    users::User,
};

use crate::{
    guards::{Authorized, BookGuard, UserGuard},
    wrap_op, DbConn, Msg,
};

const NAMESPACE: &str = "/market";

// Delete can happen if and only if the user is authorized and the product is specified
#[get("/delete")]
pub async fn delete_book(
    _auth: Authorized,
    book: BookGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    wrap_op(
        conn.run(move |c| Products::delete_by_id(c, book.book.get_id()))
            .await,
        NAMESPACE,
    )?;
    Ok(Redirect::to(NAMESPACE))
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
// Notice that we have to then redirect users on post_book page to user portal if they are not logged in

// Update the book, this is more specific than creation, meaning that it should be routed first
#[post("/cow_book", data = "<info>", rank = 1)]
pub async fn update_book(
    mut book: BookGuard,
    _user: UserGuard,
    _auth: Authorized,
    info: Form<BookInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book_id = book.book.get_id().to_string();
    // The user is the seller, he/she is authorized
    book.book.update(info.into_inner().into());
    wrap_op(
        conn.run(move |c| Products::update(c, book.book)).await,
        NAMESPACE,
    )?;
    Ok(Redirect::to(format!(
        "/market/book_info?book_id={}",
        book_id,
    )))
}

// User is logged in, creating the book.
#[post("/cow_book", data = "<info>", rank = 2)]
pub async fn create_book(
    user: UserGuard,
    info: Form<BookInfo>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let product_id = wrap_op(
        conn.run(move |c| {
            Products::create(
                c,
                user.user.get_id(),
                &info.category,
                &info.prodname,
                info.price,
                &info.description,
            )
        })
        .await,
        NAMESPACE,
    )?;
    Ok(Redirect::to(format!(
        "/market/book_info?book_id={}",
        product_id
    )))
}

#[derive(Template)]
#[template(path = "market/update_book.html")]
pub struct UpdateBook {
    book: Product,
    categories: Vec<Category>,
}

// If there is a book specified, we then use the default value of that specified book for update
#[get("/post_book", rank = 1)]
pub async fn update_book_page(
    conn: DbConn,
    _user: UserGuard,
    _auth: Authorized,
    book: BookGuard,
) -> Result<UpdateBook, Flash<Redirect>> {
    Ok(UpdateBook {
        // If there is no leaves, user cannot create any books, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: wrap_op(
            conn.run(move |c| Categories::list_leaves(c)).await,
            uri!("/market", all_books: _),
        )?,
        book: book.book,
    })
}

#[derive(Template)]
#[template(path = "market/post_book.html")]
pub struct PostBook {
    categories: Vec<Category>,
}

// post_book page
// If there is a book specified, we then use the default value of that specified book for update
#[get("/post_book", rank = 2)]
pub async fn post_book_page(conn: DbConn, _user: UserGuard) -> Result<PostBook, Flash<Redirect>> {
    Ok(PostBook {
        // If there is no leaves, user cannot create any books, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: wrap_op(
            conn.run(move |c| Categories::list_leaves(c)).await,
            uri!("/market", all_books: _),
        )?,
    })
}

#[get("/post_book", rank = 3)]
pub async fn post_book_error_page() -> Flash<Redirect> {
    Flash::error(
        Redirect::to("/user"),
        "please check if you have logged in and authorized to update/create",
    )
}

#[derive(Template)]
#[template(path = "market/book_info_owned.html")]
pub struct BookPageOwned {
    book: Product,
}

#[derive(Template)]
#[template(path = "market/book_info_user.html")]
pub struct BookPageUser {
    book: Product,
    seller: User,
}

#[derive(Template)]
#[template(path = "market/book_info_guest.html")]
pub struct BookPageGuest {
    book: Product,
}

// If the seller is the user, buttons like update and delete are displayed
#[get("/book_info", rank = 1)]
pub async fn book_page_owned(
    book: BookGuard,
    _user: UserGuard,
    _auth: Authorized,
) -> BookPageOwned {
    BookPageOwned { book: book.book }
}

// If the user is signed in but not authorized, book information and seller information will be displayed
#[get("/book_info", rank = 2)]
pub async fn book_page_user(book: BookGuard, _user: UserGuard) -> BookPageUser {
    BookPageUser {
        book: book.book,
        seller: book.seller,
    }
}

// If the user is not signed in, only book information will be displayed
#[get("/book_info", rank = 3)]
pub async fn book_page_guest(book: BookGuard) -> BookPageGuest {
    BookPageGuest { book: book.book }
}

// If the book is not specified, error id returned
#[get("/book_info", rank = 4)]
pub async fn book_page_error() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(NAMESPACE),
        "no book found with the given book ID",
    )
}

#[derive(Template)]
#[template(path = "market/categories.html")]
pub struct CategoriesPage {
    categories: Vec<Category>,
}

// If there is no category specified, we simply go for the top categories
#[get("/categories", rank = 2)]
pub async fn categories_all(conn: DbConn) -> Result<CategoriesPage, Flash<Redirect>> {
    wrap_op(conn.run(move |c| Categories::list_top(c)).await, NAMESPACE)
        .map(|v| CategoriesPage { categories: v })
}

// Category browsing
#[get("/categories?<ctg>", rank = 1)]
pub async fn categories(
    conn: DbConn,
    ctg: String,
) -> Result<Result<CategoriesPage, Redirect>, Flash<Redirect>> {
    // There is a specified category name
    let ctg_cloned = ctg.clone();
    let category = wrap_op(
        conn.run(move |c| Categories::find_by_id(c, &ctg_cloned))
            .await,
        NAMESPACE,
    )?;

    // The category is a leaf, meaning that we then have to search for books related to that
    if category.is_leaf() {
        Ok(Err(Redirect::to(uri!("/market", all_books: Some(ctg)))))
    } else {
        // The category is not a leaf, continuing down the path
        Ok(Ok(CategoriesPage {
            categories: wrap_op(
                conn.run(move |c| Categories::subcategory(c, &ctg)).await,
                NAMESPACE,
            )?,
        }))
    }
}

#[derive(Template)]
#[template(path = "market/all_books.html")]
pub struct AllBooks {
    books: Vec<Product>,
    inner: crate::Msg,
}

// List all products
#[get("/all_books?<category>")]
pub async fn all_books(
    conn: DbConn,
    category: Option<String>,
    flash: Option<FlashMessage<'_>>,
) -> Result<AllBooks, Flash<Redirect>> {
    Ok(AllBooks {
        books: if let Some(name) = category {
            wrap_op(
                conn.run(move |c| ProductFinder::new(c, None).category(&name).search())
                    .await,
                "/",
            )?
        } else {
            // Default with all products
            wrap_op(conn.run(move |c| Products::list(c)).await, "/")?
        },
        inner: Msg::from_flash(flash),
    })
}

#[get("/")]
pub async fn market() -> Redirect {
    Redirect::to(uri!("/market", all_books: _))
}
