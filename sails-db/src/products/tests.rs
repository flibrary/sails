use super::*;
use crate::{categories::Category, tags::*, test_utils::establish_connection, users::*, Cmp};
use std::collections::HashMap;

#[test]
fn create_product() {
    let conn = establish_connection();
    // our seller
    let user_id = UserForm::new("TestUser@example.org", "NFLS", "", None)
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

    // The book category
    let econ = Category::create(&conn, "Economics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        700, // CNY 700 each
        1,   // Only one book available
        "A very great book on the subject of Economics",
        Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();
    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 1);
}

#[test]
fn search_products() {
    let conn = establish_connection();
    // our seller
    let user_id = UserForm::new("TestUser@example.org", "NFLS", "", None)
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

    // The book category
    let mut books = Category::create(&conn, "Books", 1).unwrap();
    let mut econ = Category::create(&conn, "Economics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();
    let mut phys = Category::create(&conn, "Physics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();

    econ.insert(&conn, &mut books).unwrap();
    phys.insert(&conn, &mut books).unwrap();

    // Non-leaf categories are not allowed to insert
    /*
    IncompleteProduct::new(
        &books.into_leaf().unwrap(),
        "Krugman's Economics 2nd Edition",
        700,
        "A very great book on the subject of Economics",
    )
    .create(&conn, &user_id);
    */

    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        700,
        1,
        "A very great book on the subject of Economics",
        Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        500,
        1,
        "A very great book on the subject of Economics",
        Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        600,
        1,
        "That is a bad book though",
        Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Another different economics book
    IncompleteProduct::new(
        &econ,
        "The Economics",
        600,
        1,
        "I finally had got a different econ textbook!",
        Currency::USD,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Feynman's Lecture on Physics!
    IncompleteProduct::new(
        &phys,
        "Feynman's Lecture on Physics",
        900,
        1,
        "A very masterpiece on the theory of the universe",
        Currency::JPY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Search lower than CNY 300 Feynman's Lecture on Physics
    assert_eq!(
        ProductFinder::new(&conn, None)
            .prodname("Feynman's Lecture on Physics")
            .price(300, Cmp::LessThan)
            .search()
            .unwrap()
            .len(),
        0
    );

    // Search higher than CNY 300 Feynman's Lecture on Physics
    assert_eq!(
        ProductFinder::new(&conn, None)
            .prodname("Feynman's Lecture on Physics")
            .price(300, Cmp::GreaterThan)
            .search()
            .unwrap()
            .len(),
        1
    );

    // Krugman
    assert_eq!(
        ProductFinder::new(&conn, None)
            .prodname("Krugman's Economics 2nd Edition")
            .price(550, Cmp::GreaterThan)
            .search()
            .unwrap()
            .len(),
        2
    );

    // Search by category
    assert_eq!(
        ProductFinder::new(&conn, None)
            .category(&econ)
            .unwrap()
            .price(550, Cmp::GreaterThan)
            .search()
            .unwrap()
            .len(),
        3
    );

    assert_eq!(
        ProductFinder::new(&conn, None)
            .category(&books)
            .unwrap()
            .price(550, Cmp::GreaterThan)
            .search()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn delete_product() {
    let conn = establish_connection();
    let builder = TagsBuilder::new(HashMap::new());
    builder.build(&conn).unwrap();

    // our seller
    let user_id = UserForm::new("TestUser@example.org", "NFLS", "", None)
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

    // The book category
    let econ = Category::create(&conn, "Economics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();
    let id = IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        600,
        1,
        "That is a bad book though",
        Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    let sales = Tags::find_by_id(&conn, "sales").unwrap();
    TagMapping::create(&conn, &sales, &id).unwrap();

    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 1);
    assert_eq!(TagMappingFinder::new(&conn, None).count().unwrap(), 1);
    id.delete(&conn).unwrap();
    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 0);
    assert_eq!(TagMappingFinder::new(&conn, None).count().unwrap(), 0);
}

#[test]
fn product_status() {
    let conn = establish_connection();
    // our seller
    let user_id = UserForm::new("TestUser@example.org", "NFLS", "", None)
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

    // The book category
    let econ = Category::create(&conn, "Economics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();
    let id = IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        600,
        1,
        "That is a bad book though",
        Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    id.get_info(&conn)
        .unwrap()
        .set_product_status(ProductStatus::Disabled)
        .update(&conn)
        .unwrap();

    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 1);
    assert_eq!(
        ProductFinder::new(&conn, None)
            .allowed()
            .search()
            .unwrap()
            .len(),
        0
    );
}
