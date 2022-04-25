use super::{Digicon, DigiconBuilder, DigiconMapping, Digicons};
use crate::{
    categories::*, error::SailsDbError, products::*, tags::TagMappingFinder,
    test_utils::establish_connection, transactions::Transactions, users::*,
};
use std::collections::HashMap;

#[test]
fn create_digicons() {
    let conn = establish_connection();

    let physics_done_wrong = Digicon::create(
        &conn,
        "eea1dc23-5494-4293-8c48-e03c168aad8e",
        "GitHub",
        "https://github.com",
    )
    .unwrap();

    assert!(matches!(
        Digicon::create(
            &conn,
            "eea1dc23-5494-4293-8c48-e03c168aad8e",
            "GitHub",
            "https://github.com",
        )
        .err()
        .unwrap(),
        SailsDbError::DigiconExisted
    ));
    assert_eq!(ads.get_id(), "eea1dc23-5494-4293-8c48-e03c168aad8e");
    assert_eq!(ads.get_name(), "GitHub");
    assert_eq!(ads.get_link(), "https://github.com");
}

#[test]
fn mapping() {
    let conn = establish_connection();
    let physics_done_wrong = Digicon::create(
        &conn,
        "eea1dc23-5494-4293-8c48-e03c168aad8e",
        "GitHub",
        "https://github.com",
    )
    .unwrap();

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

    // The category
    let store = Category::create(&conn, "Store", 1)
        .and_then(Category::into_leaf)
        .unwrap();

    let subscription = IncompleteProduct::new(
        &store,
        "FLibrary One Subscription (1 month)",
        10, // CNY 10 each
        999,
        "Everything you love in FLibrary at an unbelievable price",
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    DigiconMapping::create(&conn, &physics_done_wrong, &subscription).unwrap();

    assert_eq!(
        DigiconMappingFinder::is_authorized(&conn, &user_id, &physics_done_wrong),
        Ok(false)
    );

    let tx = Transactions::buy(
        &conn,
        &subscription,
        &user_id,
        1,
        "258 Huanhu South Road, Dongqian Lake, Ningbo, China",
    )
    .err()
    .unwrap();

    assert_eq!(
        DigiconMappingFinder::is_authorized(&conn, &user_id, &physics_done_wrong),
        Ok(true)
    );

    tx.refund(&conn).unwrap();

    assert_eq!(
        DigiconMappingFinder::is_authorized(&conn, &user_id, &physics_done_wrong),
        Ok(false)
    );
}
