use super::{User, Users};
use crate::test_utils::establish_connection;

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
    assert_eq!(Users::list(&conn).unwrap().len(), 1);
    Users::delete_by_id(&conn, &user.id).unwrap();
    assert_eq!(Users::list(&conn).unwrap().len(), 0);
}

#[test]
fn update_user() {
    let conn = establish_connection();
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user.clone()).unwrap();

    let mut user_returned = Users::find_by_id(&conn, &user.id).unwrap();
    user_returned.change_passwd("SomeStrongPasswd").unwrap();
    user_returned.school = "University of Cambridge".to_string();

    Users::create_or_update(&conn, user_returned).unwrap();
    let user_changed = Users::find_by_id(&conn, &user.id).unwrap();
    assert_eq!(&user_changed.school, "University of Cambridge");
    // Unchanged fields should stay the same (after conversion)
    assert_eq!(&user_changed.phone, "+8618353232340");
    assert_eq!(
        user_changed.verify_passwd("SomeStrongPasswd").unwrap(),
        true,
    );
    assert_eq!(Users::list(&conn).unwrap().len(), 1);
}
