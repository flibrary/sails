use askama::Template;
use comrak::{markdown_to_html, ComrakOptions};
use once_cell::sync::Lazy;
use rocket::{
    form::Form,
    request::FlashMessage,
    response::{Flash, Redirect},
};
use sails_db::{categories::*, error::SailsDbError, products::*, users::*};

use crate::{guards::*, wrap_op, DbConn, Msg};

// Comrak options. We selectively enabled a few GFM standards.
static COMRAK_OPT: Lazy<ComrakOptions> = Lazy::new(|| {
    let mut opts = ComrakOptions::default();
    opts.extension.table = true;
    opts.extension.tasklist = true;
    opts.extension.strikethrough = true;
    opts.extension.autolink = true;
    opts.extension.footnotes = true;
    opts
});

// Delete can happen if and only if the user is authorized and the product is specified
#[get("/delete")]
pub async fn delete_book(
    _auth: Authorized,
    book: BookIdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    wrap_op(
        conn.run(move |c| book.book_id.delete(c)).await,
        uri!("/market", market),
    )?;
    Ok(Redirect::to(uri!("/market", market)))
}

// Handle book creation or update
// If the product is unspecified, then we are in creating mode, else we are updating
// For either creating a book or updating a book, the user must be signed in.
// For updating a book, the user must additionally be authorized
// Notice that we have to then redirect users on post_book page to user portal if they are not logged in

// Update the book, this is more specific than creation, meaning that it should be routed first
#[post("/cow_book", data = "<info>", rank = 1)]
pub async fn update_book(
    book: BookIdGuard,
    _user: UserIdGuard,
    _auth: Authorized,
    info: Form<IncompleteProductOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book_id = book.book_id.get_id().to_string();
    // The user is the seller, he/she is authorized
    wrap_op(
        conn.run(move |c| book.book_id.update_owned(c, info.into_inner()))
            .await,
        uri!("/market", market),
    )?;
    Ok(Redirect::to(format!(
        "/market/book_info?book_id={}",
        book_id,
    )))
}

// User is logged in, creating the book.
#[post("/cow_book", data = "<info>", rank = 2)]
pub async fn create_book(
    user: UserIdGuard,
    info: Form<IncompleteProductOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let product_id = wrap_op(
        conn.run(move |c| {
            IncompleteProduct::new(
                &info.category,
                &info.prodname,
                info.price,
                &info.description,
            )
            .create(c, &user.id)
        })
        .await,
        uri!("/market", market),
    )?;
    Ok(Redirect::to(format!(
        "/market/book_info?book_id={}",
        product_id.get_id()
    )))
}

#[derive(Template)]
#[template(path = "market/update_book.html")]
pub struct UpdateBook {
    book: ProductInfo,
    categories: Vec<Category>,
}

// If there is a book specified, we then use the default value of that specified book for update
#[get("/post_book", rank = 1)]
pub async fn update_book_page(
    conn: DbConn,
    // Can we remove this guard
    _user: UserIdGuard,
    _auth: Authorized,
    book: BookInfoGuard,
) -> Result<UpdateBook, Flash<Redirect>> {
    Ok(UpdateBook {
        // If there is no leaves, user cannot create any books, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: wrap_op(
            conn.run(move |c| Categories::list_leaves(c)).await,
            uri!("/market", all_books(_)),
        )?,
        book: book.book_info,
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
pub async fn post_book_page(conn: DbConn, _user: UserIdGuard) -> Result<PostBook, Flash<Redirect>> {
    Ok(PostBook {
        // If there is no leaves, user cannot create any books, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: wrap_op(
            conn.run(move |c| Categories::list_leaves(c)).await,
            uri!("/market", all_books(_)),
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
    book: ProductInfo,
    category: Category,
    desc_rendered: String,
}

#[derive(Template)]
#[template(path = "market/book_info_user.html")]
pub struct BookPageUser {
    book: ProductInfo,
    category: Category,
    desc_rendered: String,
    seller: UserInfo,
}

#[derive(Template)]
#[template(path = "market/book_info_guest.html")]
pub struct BookPageGuest {
    book: ProductInfo,
    category: Category,
    desc_rendered: String,
}

// If the seller is the user, buttons like update and delete are displayed
#[get("/book_info", rank = 1)]
pub async fn book_page_owned(book: BookInfoGuard, _auth: Authorized) -> BookPageOwned {
    let rendered = markdown_to_html(book.book_info.get_description(), &COMRAK_OPT);
    BookPageOwned {
        book: book.book_info,
        category: book.category,
        desc_rendered: rendered,
    }
}

// If the user is signed in but not authorized, book information and seller information will be displayed
#[get("/book_info", rank = 2)]
pub async fn book_page_user(book: BookInfoGuard, _user: UserIdGuard) -> BookPageUser {
    let rendered = markdown_to_html(book.book_info.get_description(), &COMRAK_OPT);
    BookPageUser {
        book: book.book_info,
        category: book.category,
        desc_rendered: rendered,
        seller: book.seller_info,
    }
}

// If the user is not signed in, only book information will be displayed
#[get("/book_info", rank = 3)]
pub async fn book_page_guest(book: BookInfoGuard) -> BookPageGuest {
    let rendered = markdown_to_html(book.book_info.get_description(), &COMRAK_OPT);
    BookPageGuest {
        book: book.book_info,
        category: book.category,
        desc_rendered: rendered,
    }
}

// If the book is not specified, error id returned
#[get("/book_info", rank = 4)]
pub async fn book_page_error() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/market", market)),
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
    wrap_op(
        conn.run(move |c| Categories::list_top(c)).await,
        uri!("/market", market),
    )
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
        uri!("/market", market),
    )?;

    // The category is a leaf, meaning that we then have to search for books related to that
    if category.is_leaf() {
        Ok(Err(Redirect::to(uri!("/market", all_books(Some(ctg))))))
    } else {
        // The category is not a leaf, continuing down the path
        Ok(Ok(CategoriesPage {
            categories: wrap_op(
                conn.run(move |c| Categories::subcategory(c, &ctg)).await,
                uri!("/market", market),
            )?,
        }))
    }
}

#[derive(Template)]
#[template(path = "market/all_books.html")]
pub struct AllBooks {
    books: Vec<(ProductInfo, Category)>,
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
        books: conn
            .run(
                move |c| -> Result<Vec<(ProductInfo, Category)>, SailsDbError> {
                    let book_info = if let Some(id) = category {
                        ProductFinder::new(c, None).category(&id).search_info()?
                    } else {
                        ProductFinder::list_info(c)?
                    };

                    book_info
                        .into_iter()
                        .map(|x| {
                            let ctg = Categories::find_by_id(c, x.get_category()).unwrap();
                            Ok((x, ctg))
                        })
                        .collect()
                },
            )
            .await
            .unwrap(),
        inner: Msg::from_flash(flash),
    })
}

#[get("/")]
pub async fn market() -> Redirect {
    Redirect::to(uri!("/market", all_books(_)))
}
