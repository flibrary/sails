use super::Users;
use crate::{
    categories::Categories, products::Products, test_utils::establish_connection, users::User,
};

#[test]
fn create_user() {
    let conn = establish_connection();
    Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    assert_eq!(Users::list(&conn).unwrap().len(), 1);
}

#[test]
fn register_user() {
    let conn = establish_connection();
    Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    // User already registered
    assert!(Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .is_err());
}

#[test]
fn login_user() {
    let conn = establish_connection();
    Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    assert_eq!(Users::list(&conn).unwrap().len(), 1);

    assert!(Users::login(&conn, "TestUser@example.org", "strongpasswd").is_ok());
}

#[test]
fn delete_user() {
    let conn = establish_connection();
    let user = Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();

    let another_user = Users::register(
        &conn,
        "TestUser2@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    let econ_id = Categories::create(&conn, "Economics").unwrap();
    Products::create(
        &conn,
        user.as_str(),
        econ_id.as_str(),
        "Economics",
        1,
        "A horrible book",
    )
    .unwrap();

    Products::create(
        &conn,
        user.as_str(),
        econ_id.as_str(),
        "The Economics",
        1,
        "Another horrible book",
    )
    .unwrap();

    Products::create(
        &conn,
        another_user.as_str(),
        econ_id.as_str(),
        "Economics Principle",
        1,
        "Another horrible book",
    )
    .unwrap();
    assert_eq!(Products::list(&conn).unwrap().len(), 3);
    assert_eq!(Users::list(&conn).unwrap().len(), 2);
    Users::delete_by_id(&conn, &user).unwrap();
    // There is still one book created by TestUser2
    assert_eq!(Products::list(&conn).unwrap().len(), 1);
    // Only TestUser2 is left
    assert_eq!(Users::list(&conn).unwrap().len(), 1);
}

#[test]
fn update_user() {
    let conn = establish_connection();

    let fake_user = User::new(
        "a@example.org",
        "Fake School",
        "+8618368203450",
        "abadpasswd",
    )
    .unwrap();
    // User doesn't exist
    assert!(Users::update(&conn, fake_user).is_err());

    let user_id = Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();
    let another_user = Users::register(
        &conn,
        "AnotherUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();

    // Let's say that on some day a user wants to change the school or password
    let mut user_returned = Users::find_by_id(&conn, &user_id).unwrap();
    user_returned.change_passwd("SomeStrongPasswd").unwrap();
    user_returned.school = "University of Cambridge".to_string();
    Users::update(&conn, user_returned).unwrap();

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
