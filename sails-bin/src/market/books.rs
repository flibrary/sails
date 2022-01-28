use crate::{guards::*, sanitize_html, DbConn, IntoFlash};
use askama::Template;
use rocket::{
    form::Form,
    response::{Flash, Redirect},
};
use sails_db::{categories::*, error::SailsDbError, products::*, users::*};

// Delete can happen if and only if the user is authorized and the product is specified
#[get("/delete?<book_id>")]
pub async fn delete_book(
    _auth: Auth<BookRemovable>,
    book_id: BookGuard,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_id(&conn).await.into_flash(uri!("/"))?;
    conn.run(move |c| book.book_id.delete(c))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/market", super::market)))
}

// Handle book creation or update
// If the product is unspecified, then we are in creating mode, else we are updating
// For either creating a book or updating a book, the user must be signed in.
// For updating a book, the user must additionally be authorized
// Notice that we have to then redirect users on post_book page to user portal if they are not logged in

// Update the book, this is more specific than creation, meaning that it should be routed first
#[post("/cow_book?<book_id>", data = "<info>", rank = 1)]
pub async fn update_book(
    book_id: BookGuard,
    _auth: Auth<BookWritable>,
    mut info: Form<IncompleteProductOwned>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let book = book_id.to_id(&conn).await.into_flash(uri!("/"))?;
    info.description = sanitize_html(&info.description);
    // The user is the seller, he/she is authorized
    conn.run(move |c| book.book_id.update_owned(c, info.into_inner().verify(c)?))
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!("/market", book_page_owned(book_id))))
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
    Ok(Redirect::to(uri!(
        "/market",
        instruction(product_id.get_id())
    )))
}

#[derive(Template)]
#[template(path = "market/update_book.html")]
pub struct UpdateBook {
    book: ProductInfo,
    categories: Vec<Category>,
}

// If there is a book specified, we then use the default value of that specified book for update
#[get("/post_book?<book_id>", rank = 1)]
pub async fn update_book_page(
    conn: DbConn,
    _auth: Auth<BookWritable>,
    book_id: BookGuard,
) -> Result<UpdateBook, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
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
#[template(path = "market/post_book_interim.html")]
pub struct PostBookInterim;

#[get("/post_book_interim")]
pub async fn post_book_interim() -> PostBookInterim {
    PostBookInterim
}

#[derive(Template)]
#[template(path = "market/delegate_book.html")]
pub struct DelegateBookPage {
    categories: Vec<Category>,
}

// Required to sign in
#[get("/delegate_book", rank = 1)]
pub async fn delegate_book_page(
    conn: DbConn,
    _user: UserIdGuard<Cookie>,
) -> Result<DelegateBookPage, Flash<Redirect>> {
    Ok(DelegateBookPage {
        // If there is no leaves, user cannot create any books, a message should be displayed inside the template
        // TODO: categories should only be fetched once
        categories: conn
            .run(move |c| Categories::list_leaves(c))
            .await
            .into_flash(uri!("/"))?,
    })
}

#[get("/delegate_book", rank = 2)]
pub async fn delegate_book_error_page() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/")),
        "please check if you have logged in and authorized to update/create",
    )
}

#[derive(FromForm)]
pub struct Delegation {
    category: String,
}

#[post("/delegate_book", data = "<info>")]
pub async fn delegate_book(
    user: UserIdGuard<Cookie>,
    info: Form<Delegation>,
    conn: DbConn,
) -> Result<Redirect, Flash<Redirect>> {
    let category = conn
        .run(move |c| Categories::find_by_id(c, &info.category))
        .await
        .into_flash(uri!("/"))?
        .into_leaf()
        .into_flash(uri!("/"))?;

    let info = IncompleteProductOwned::new(&category, category.name(), category.get_price(), 1, "")
        .into_flash(uri!("/"))?;

    let product_id = conn
        .run(move |c| -> Result<ProductId, SailsDbError> {
            info.create(
                c,
                &user.id,
                &UserFinder::new(c, None)
                    .id("flibrarynfls@outlook.com")
                    .first()?,
            )
        })
        .await
        .into_flash(uri!("/"))?;
    Ok(Redirect::to(uri!(
        "/market",
        instruction(product_id.get_id())
    )))
}

#[derive(Template)]
#[template(path = "market/instruction.html")]
pub struct InstructionPage {
    // Id is the first part of the UUID, written in decimal
    info: ProductInfo,
}

// This page should be restricted to readable only because people who delegate books get redirected to this page.
#[get("/instruction?<book_id>")]
pub async fn instruction(
    book_id: BookGuard,
    conn: DbConn,
    _auth: Auth<BookReadable>,
) -> Result<InstructionPage, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(InstructionPage {
        info: book.book_info,
    })
}

#[derive(Template)]
#[template(path = "market/book_info_owned.html")]
pub struct BookPageOwned {
    book: ProductInfo,
    category: Option<LeafCategory>,
    seller: UserInfo,
}

#[derive(Template)]
#[template(path = "market/book_info_user.html")]
pub struct BookPageUser {
    book: ProductInfo,
    category: Option<LeafCategory>,
    seller: UserInfo,
}

#[derive(Template)]
#[template(path = "market/book_info_guest.html")]
pub struct BookPageGuest {
    book: ProductInfo,
    category: Option<LeafCategory>,
}

// If the seller is the user, buttons like update and delete are displayed
#[get("/book_info?<book_id>", rank = 1)]
pub async fn book_page_owned(
    book_id: BookGuard,
    conn: DbConn,
    _auth: Auth<BookWritable>,
) -> Result<BookPageOwned, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(BookPageOwned {
        book: book.book_info,
        category: book
            .category
            .map(|x| x.into_leaf().into_flash(uri!("/")))
            .transpose()?,
        seller: book.seller_info,
    })
}

// If the user is signed in but not authorized, book information and seller information will be displayed
#[get("/book_info?<book_id>", rank = 2)]
pub async fn book_page_user(
    book_id: BookGuard,
    conn: DbConn,
    _auth: Auth<BookReadable>,
) -> Result<BookPageUser, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(BookPageUser {
        book: book.book_info,
        category: book
            .category
            .map(|x| x.into_leaf().into_flash(uri!("/")))
            .transpose()?,
        seller: book.seller_info,
    })
}

// If the user is not signed in, only book information will be displayed
#[get("/book_info?<book_id>", rank = 3)]
pub async fn book_page_guest(
    book_id: BookGuard,
    conn: DbConn,
) -> Result<BookPageGuest, Flash<Redirect>> {
    let book = book_id.to_info(&conn).await.into_flash(uri!("/"))?;
    Ok(BookPageGuest {
        book: book.book_info,
        category: book
            .category
            .map(|x| x.into_leaf().into_flash(uri!("/")))
            .transpose()?,
    })
}

// If the book is not specified, error id returned
#[get("/book_info", rank = 4)]
pub async fn book_page_error() -> Flash<Redirect> {
    Flash::error(
        Redirect::to(uri!("/")),
        "no book found with the given book ID",
    )
}
