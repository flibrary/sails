use std::num::NonZeroI64;

use super::{ProductFinder, Products};
use crate::{
    test_utils::establish_connection,
    users::{User, Users},
    Cmp,
};

#[test]
fn create_product() {
    let conn = establish_connection();
    // our seller
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user.clone()).unwrap();
    Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(700).unwrap(),
        "A very great book on the subject of Economics",
    )
    .unwrap();
    assert_eq!(Products::list(&conn).unwrap().len(), 1);
}

#[test]
fn search_products() {
    let conn = establish_connection();
    // our seller
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user.clone()).unwrap();
    Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(700).unwrap(),
        "A very great book on the subject of Economics",
    )
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(500).unwrap(),
        "A very great book on the subject of Economics",
    )
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(600).unwrap(),
        "That is a bad book though",
    )
    .unwrap();

    // Feynman's Lecture on Physics!
    Products::create_product(
        &conn,
        &user,
        "Feynman's Lecture on Physics",
        NonZeroI64::new(900).unwrap(),
        "A very masterpiece on the theory of the universe",
    )
    .unwrap();

    // Search lower than CNY 300 Feynman's Lecture on Physics
    assert_eq!(
        ProductFinder::new(&conn, None)
            .prodname("Feynman's Lecture on Physics")
            .price(NonZeroI64::new(300).unwrap(), Cmp::LessThan)
            .search()
            .unwrap()
            .len(),
        0
    );

    // Search higher than CNY 300 Feynman's Lecture on Physics
    assert_eq!(
        ProductFinder::new(&conn, None)
            .prodname("Feynman's Lecture on Physics")
            .price(NonZeroI64::new(300).unwrap(), Cmp::GreaterThan)
            .search()
            .unwrap()
            .len(),
        1
    );

    // Krugman
    assert_eq!(
        ProductFinder::new(&conn, None)
            .prodname("Krugman's Economics 2nd Edition")
            .price(NonZeroI64::new(550).unwrap(), Cmp::GreaterThan)
            .search()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn delete_product() {
    let conn = establish_connection();
    // our seller
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user.clone()).unwrap();
    let id = Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(700).unwrap(),
        "A very great book on the subject of Economics",
    )
    .unwrap();
    assert_eq!(Products::list(&conn).unwrap().len(), 1);
    Products::delete_by_id(&conn, &id).unwrap();
    assert_eq!(Products::list(&conn).unwrap().len(), 0);
}
