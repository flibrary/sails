use super::*;
use crate::{categories::Category, products::*, test_utils::establish_connection};

#[test]
fn create_user() {
    let conn = establish_connection();
    UserForm::new(
        "TestUser@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();
    assert_eq!(UserFinder::list(&conn).unwrap().len(), 1);
}

#[test]
fn create_user_existed() {
    let conn = establish_connection();
    UserForm::new(
        "TestUser@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    // User already registered
    // Comparison should be case-insensitive
    assert!(
        UserForm::new("testUser@example.org", "Mick Zhang", "NFLS", "strongpasswd",)
            .to_ref()
            .unwrap()
            .create(&conn)
            .is_err()
    );
}

#[test]
fn login_user() {
    let conn = establish_connection();
    UserForm::new(
        "TestUser@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    assert!(UserId::login(&conn, "TestUser@example.org", "strongpasswd").is_ok());
}

#[test]
fn delete_user() {
    let conn = establish_connection();
    let user = UserForm::new(
        "TestUser@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
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
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    let econ = Category::create(&conn, "Economics")
        .and_then(Category::into_leaf)
        .unwrap();
    IncompleteProduct::new(&econ, "Economics", 1, "A horrible book")
        .create(&conn, &user)
        .unwrap();

    IncompleteProduct::new(&econ, "The Economics", 1, "Another horrible book")
        .create(&conn, &user)
        .unwrap();

    IncompleteProduct::new(&econ, "Economics Principle", 1, "Another horrible book")
        .create(&conn, &another_user)
        .unwrap();

    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 3);
    assert_eq!(UserFinder::list(&conn).unwrap().len(), 2);
    user.delete(&conn).unwrap();
    // There is still one book created by TestUser2
    assert_eq!(ProductFinder::list(&conn).unwrap().len(), 1);
    // Only TestUser2 is left
    assert_eq!(UserFinder::list(&conn).unwrap().len(), 1);
}

#[test]
fn update_user() {
    let conn = establish_connection();

    let user_id = UserForm::new(
        "TestUser@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    let another_user = UserForm::new(
        "AnotherUser@example.org",
        "Kanyang Ying",
        "NFLS",
        "strongpasswd",
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    // Let's say that on some day a user wants to change the school or password
    user_id
        .get_info(&conn)
        .unwrap()
        .set_password("SomeStrongPasswd")
        .unwrap()
        .set_school("University of Cambridge")
        .set_user_status(UserStatus::Admin)
        .update(&conn)
        .unwrap();

    let user_changed = user_id.get_info(&conn).unwrap();
    assert_eq!(user_changed.get_school(), "University of Cambridge");
    assert_eq!(user_changed.get_user_status(), &UserStatus::Admin);
    // Unchanged fields should stay the same
    assert_eq!(user_changed.get_name(), "Kanyang Ying");
    assert_eq!(
        user_changed.verify_passwd("SomeStrongPasswd").unwrap(),
        true,
    );

    // Another user should be safe from the change (this was a bug before)
    assert_eq!(another_user.get_info(&conn).unwrap().get_school(), "NFLS");
    assert_eq!(
        another_user
            .get_info(&conn)
            .unwrap()
            .verify_passwd("strongpasswd")
            .unwrap(),
        true
    );
    assert_eq!(UserFinder::list(&conn).unwrap().len(), 2);
}
