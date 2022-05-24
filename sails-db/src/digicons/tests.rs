use super::{Digicon, DigiconMapping};
use crate::{
    categories::*, digicons::DigiconMappingFinder, error::SailsDbError, products::*,
    test_utils::establish_connection, transactions::Transactions, users::*,
};

#[test]
fn create_digicons() {
    let conn = establish_connection();

    let user_id = UserForm::new(
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

    let physics_done_wrong = Digicon::create(
        &conn,
        "eea1dc23-5494-4293-8c48-e03c168aad8e",
        &user_id,
        "GitHub",
        crate::enums::StorageType::ReleaseAsset,
    )
    .unwrap();

    assert!(matches!(
        Digicon::create(
            &conn,
            "eea1dc23-5494-4293-8c48-e03c168aad8e",
            &user_id,
            "GitHub",
            crate::enums::StorageType::ReleaseAsset,
        )
        .err()
        .unwrap(),
        SailsDbError::DigiconExisted
    ));
    assert_eq!(
        physics_done_wrong.get_id(),
        "eea1dc23-5494-4293-8c48-e03c168aad8e"
    );
    assert_eq!(physics_done_wrong.get_creator_id(), "TestUser@example.org");
    assert_eq!(physics_done_wrong.get_name(), "GitHub");
    assert_eq!(physics_done_wrong.get_storage_detail().is_none(), true);
}

#[test]
fn mapping() {
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

    let another = UserForm::new(
        "TestUser2@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
        None,
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    let physics_done_wrong = Digicon::create(
        &conn,
        "eea1dc23-5494-4293-8c48-e03c168aad8e",
        &user_id,
        "GitHub",
        crate::enums::StorageType::ReleaseAsset,
    )
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

    subscription
        .get_info(&conn)
        .unwrap()
        .set_product_status(crate::enums::ProductStatus::Verified)
        .update(&conn)
        .unwrap();

    DigiconMapping::create(&conn, &physics_done_wrong, &subscription).unwrap();

    assert_eq!(
        DigiconMappingFinder::authorized_to_read_by_purchase(&conn, &another, &physics_done_wrong)
            .unwrap(),
        false
    );

    let tx = Transactions::buy(
        &conn,
        &subscription,
        &another,
        1,
        "258 Huanhu South Road, Dongqian Lake, Ningbo, China",
    )
    .unwrap();

    tx.get_info(&conn)
        .unwrap()
        .set_transaction_status(crate::enums::TransactionStatus::Finished)
        .update(&conn)
        .unwrap();

    assert_eq!(
        DigiconMappingFinder::authorized_to_read_by_purchase(&conn, &another, &physics_done_wrong)
            .unwrap(),
        true
    );

    tx.refund(&conn).unwrap();

    assert_eq!(
        DigiconMappingFinder::authorized_to_read_by_purchase(&conn, &another, &physics_done_wrong)
            .unwrap(),
        false
    );
}
