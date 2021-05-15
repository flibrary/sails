use std::num::NonZeroI64;

use super::{User, Users};
use crate::{categories::Categories, products::Products, test_utils::establish_connection};

#[test]
fn create_user() {
    let conn = establish_connection();
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user).unwrap();
    assert_eq!(Users::list(&conn).unwrap().len(), 1);
}

#[test]
fn register_user() {
    let conn = establish_connection();
    Users::register(
        &conn,
        "TestUser",
        None,
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    // User already registered
    assert!(Users::register(
        &conn,
        "TestUser",
        None,
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .is_err());
}

#[test]
fn login_user() {
    let conn = establish_connection();
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user).unwrap();
    assert_eq!(Users::list(&conn).unwrap().len(), 1);

    assert!(Users::login(&conn, "TestUser", "strongpasswd")
        .unwrap()
        .is_some());
}

#[test]
fn delete_user() {
    let conn = establish_connection();
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user.clone()).unwrap();

    let another_user = Users::register(
        &conn,
        "TestUser2",
        None,
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    let econ_id = Categories::create(&conn, "Economics").unwrap();
    Products::create_product(
        &conn,
        user.id.as_str(),
        econ_id.as_str(),
        "Economics",
        NonZeroI64::new(1).unwrap(),
        "A horrible book",
    )
    .unwrap();

    Products::create_product(
        &conn,
        user.id.as_str(),
        econ_id.as_str(),
        "The Economics",
        NonZeroI64::new(1).unwrap(),
        "Another horrible book",
    )
    .unwrap();

    Products::create_product(
        &conn,
        another_user.as_str(),
        econ_id.as_str(),
        "Economics Principle",
        NonZeroI64::new(1).unwrap(),
        "Another horrible book",
    )
    .unwrap();
    assert_eq!(Products::list(&conn).unwrap().len(), 3);
    assert_eq!(Users::list(&conn).unwrap().len(), 2);
    Users::delete_by_id(&conn, &user.id).unwrap();
    // There is still one book created by TestUser2
    assert_eq!(Products::list(&conn).unwrap().len(), 1);
    // Only TestUser2 is left
    assert_eq!(Users::list(&conn).unwrap().len(), 1);
}

#[test]
fn update_user() {
    let conn = establish_connection();
    let user_id = Users::register(
        &conn,
        "TestUser",
        None,
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    let another_user = Users::register(
        &conn,
        "AnotherUser",
        None,
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();

    // Let's say that on some day a user wants to change the school or password
    let mut user_returned = Users::find_by_id(&conn, &user_id).unwrap();
    user_returned.change_passwd("SomeStrongPasswd").unwrap();
    user_returned.school = "University of Cambridge".to_string();
    Users::create_or_update(&conn, user_returned).unwrap();

    // ID should not change
    let user_changed = Users::find_by_id(&conn, &user_id).unwrap();
    assert_eq!(&user_changed.school, "University of Cambridge");
    // Unchanged fields should stay the same (after conversion)
    assert_eq!(&user_changed.phone, "+8618353232340");
    assert_eq!(
        user_changed.verify_passwd("SomeStrongPasswd").unwrap(),
        true,
    );

    // Another user should be safe from the change (this was a bug before)
    assert_eq!(
        Users::find_by_id(&conn, &another_user).unwrap().school,
        "NFLS"
    );
    assert_eq!(
        Users::find_by_id(&conn, &another_user)
            .unwrap()
            .verify_passwd("strongpasswd")
            .unwrap(),
        true
    );
    assert_eq!(Users::list(&conn).unwrap().len(), 2);
}
