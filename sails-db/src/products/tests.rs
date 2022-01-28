use super::*;
use crate::{categories::Category, test_utils::establish_connection, users::*, Cmp};

#[test]
fn create_product() {
    let conn = establish_connection();
    // our seller
    let user_id = UserForm::new(
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
        None,
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    // The book category
    let econ = Category::create(&conn, "Economics Books", 490)
        .and_then(Category::into_leaf)
        .unwrap();
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        700, // CNY 700 each
        1,   // Only one book available
        "A very great book on the subject of Economics",
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
    .unwrap();
    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 1);
}

#[test]
fn search_products() {
    let conn = establish_connection();
    // our seller
    let user_id = UserForm::new(
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
        None,
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    // The book category
    let mut books = Category::create(&conn, "Books", 1).unwrap();
    let mut econ = Category::create(&conn, "Economics Books", 490)
        .and_then(Category::into_leaf)
        .unwrap();
    let mut phys = Category::create(&conn, "Physics Books", 630)
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
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        500,
        1,
        "A very great book on the subject of Economics",
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        600,
        1,
        "That is a bad book though",
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
    .unwrap();

    // Another different economics book
    IncompleteProduct::new(
        &econ,
        "The Economics",
        600,
        1,
        "I finally had got a different econ textbook!",
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
    .unwrap();

    // Feynman's Lecture on Physics!
    IncompleteProduct::new(
        &phys,
        "Feynman's Lecture on Physics",
        900,
        1,
        "A very masterpiece on the theory of the universe",
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
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
            .price(550, Cmp::GreaterThan)
            .search()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn delete_product() {
    let conn = establish_connection();
    // our seller
    let user_id = UserForm::new(
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
        None,
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    // The book category
    let econ = Category::create(&conn, "Economics Books", 490)
        .and_then(Category::into_leaf)
        .unwrap();
    let id = IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        600,
        1,
        "That is a bad book though",
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
    .unwrap();

    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 1);
    id.delete(&conn).unwrap();
    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 0);
}

#[test]
fn product_status() {
    let conn = establish_connection();
    // our seller
    let user_id = UserForm::new(
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
        None,
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    // The book category
    let econ = Category::create(&conn, "Economics Books", 490)
        .and_then(Category::into_leaf)
        .unwrap();
    let id = IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        600,
        1,
        "That is a bad book though",
    )
    .unwrap()
    .create(&conn, &user_id, &user_id)
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

#[test]
fn delegation() {
    let conn = establish_connection();
    let user = UserForm::new(
        "TestUser@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
        None,
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    let another_user = UserForm::new(
        "TestUser2@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
        None,
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    let econ = Category::create(&conn, "Economics", 490)
        .and_then(Category::into_leaf)
        .unwrap();
    // Seller
    IncompleteProduct::new(&econ, "Economics", 1, 1, "A horrible book")
        .unwrap()
        .create(&conn, &user, &user)
        .unwrap();

    // Owner
    IncompleteProduct::new(&econ, "Economics Principle", 1, 1, "Another horrible book")
        .unwrap()
        .create(&conn, &user, &another_user)
        .unwrap();

    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 2);
    assert_eq!(
        ProductFinder::new(&conn, None)
            .seller(&user.get_id())
            .first_info()
            .unwrap()
            .prodname,
        "Economics"
    );
    assert_eq!(
        ProductFinder::new(&conn, None)
            .owner(&user.get_id())
            .first_info()
            .unwrap()
            .prodname,
        "Economics Principle"
    );
    // Another user owns nothing!
    assert_eq!(
        ProductFinder::new(&conn, None)
            .seller(&another_user.get_id())
            .search()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        ProductFinder::new(&conn, None)
            .delegator(&another_user.get_id())
            .first_info()
            .unwrap()
            .prodname,
        "Economics Principle"
    );
}
