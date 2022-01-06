use askama::Template;
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};
use sails_db::{categories::*, error::SailsDbError, products::*, users::*, Cmp};

use crate::{guards::*, sanitize_html, DbConn, IntoFlash};

// Delete can happen if and only if the user is authorized and the product is specified
#[get("/delete")]
pub async fn delete_book(
    _auth: Auth<BookRemovable>,
    book: BookIdGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    conn.run(move |c| book.book_id.delete(c))
        .await
        .into_flash(uri!("/"))?;
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
    _auth: Auth<BookWritable>,
    mut info: Form<IncompleteProductOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    info.description = sanitize_html(&info.description);
    let book_id = book.book_id.get_id().to_string();
    // The user is the seller, he/she is authorized
    conn.run(move |c| book.book_id.update_owned(c, info.into_inner().verify(c)?))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(format!(
        "/market/book_info?book_id={}",
        book_id,
    )))
}

// User is logged in, creating the book.
#[post("/cow_book", data = "<info>", rank = 2)]
pub async fn create_book(
    user: UserIdGuard<Cookie>,
    mut info: Form<IncompleteProductOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    info.description = sanitize_html(&info.description);
    let product_id = conn
        .run(move |c| info.create(c, &user.id, &user.id))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(format!(
        "/market/instruction?book_id={}",
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
    _auth: Auth<BookWritable>,
    book: BookInfoGuard<ProductInfo>,
) -> Result<UpdateBook, Flash<Redirect>> {
    Ok(UpdateBook {
        // If there is no leaves, user cannot create any books, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: conn
            .run(move |c| Categories::list_leaves(c))
            .await
            .into_flash(uri!("/"))?,
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
pub async fn post_book_page(
    conn: DbConn,
    _user: UserIdGuard<Cookie>,
) -> Result<PostBook, Flash<Redirect>> {
    Ok(PostBook {
        // If there is no leaves, user cannot create any books, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: conn
            .run(move |c| Categories::list_leaves(c))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[get("/post_book", rank = 3)]
pub async fn post_book_error_page() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/")),
        "please check if you have logged in and authorized to update/create",
    )
}

#[derive(Template)]
#[template(path = "market/instruction.html")]
pub struct InstructionPage {
    // Id is the first part of the UUID, written in decimal
    info: ProductInfo,
}

#[get("/instruction")]
pub async fn instruction(
    book: BookInfoGuard<ProductInfo>,
    _auth: Auth<BookWritable>,
) -> Result<InstructionPage, Flash<Redirect>> {
    Ok(InstructionPage {
        info: book.book_info,
    })
}

#[derive(Template)]
#[template(path = "market/book_info_owned.html")]
pub struct BookPageOwned {
    book: ProductInfo,
    category: Option<Category>,
    seller: UserInfo,
}

#[derive(Template)]
#[template(path = "market/book_info_user.html")]
pub struct BookPageUser {
    book: ProductInfo,
    category: Option<Category>,
    seller: UserInfo,
}

#[derive(Template)]
#[template(path = "market/book_info_guest.html")]
pub struct BookPageGuest {
    book: ProductInfo,
    category: Option<Category>,
}

// If the seller is the user, buttons like update and delete are displayed
#[get("/book_info", rank = 1)]
pub async fn book_page_owned(
    book: BookInfoGuard<ProductInfo>,
    _auth: Auth<BookWritable>,
) -> BookPageOwned {
    BookPageOwned {
        book: book.book_info,
        category: book.category,
        seller: book.seller_info,
    }
}

// If the user is signed in but not authorized, book information and seller information will be displayed
#[get("/book_info", rank = 2)]
pub async fn book_page_user(
    book: BookInfoGuard<ProductInfo>,
    _auth: Auth<BookReadable>,
) -> BookPageUser {
    BookPageUser {
        book: book.book_info,
        category: book.category,
        seller: book.seller_info,
    }
}

// If the user is not signed in, only book information will be displayed
#[get("/book_info", rank = 3)]
pub async fn book_page_guest(book: BookInfoGuard<ProductInfo>) -> BookPageGuest {
    BookPageGuest {
        book: book.book_info,
        category: book.category,
    }
}

// If the book is not specified, error id returned
#[get("/book_info", rank = 4)]
pub async fn book_page_error() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/")),
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
