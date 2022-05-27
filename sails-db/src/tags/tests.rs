use super::{Tag, TagMapping, Tags, TagsBuilder};
use crate::{
    categories::*, error::SailsDbError, products::*, tags::TagMappingFinder,
    test_utils::establish_connection, users::*,
};
use std::collections::HashMap;

#[test]
fn create_tags() {
    let conn = establish_connection();

    let ads = Tag::create(
        &conn,
        "ads",
        "广告",
        Some("<h1>广告</h1>"),
        Option::<&str>::None,
    )
    .unwrap();

    assert!(matches!(
        Tag::create(
            &conn,
            "ads",
            "广告2",
            Some("<h1>广告</h1>"),
            Option::<&str>::None,
        )
        .err()
        .unwrap(),
        SailsDbError::TagExisted
    ));
    assert_eq!(ads.get_name(), "广告");
}

#[test]
fn tagsbuilder() {
    let conn = establish_connection();
    let builder = TagsBuilder::new(HashMap::new());
    builder.build(&conn).unwrap();

    let ads = Tags::find_by_id(&conn, "ads").unwrap();
    let sales = Tags::find_by_id(&conn, "sales").unwrap();
    assert_eq!(ads.get_name(), "广告");
    assert_eq!(sales.get_name(), "特别优惠");
}

#[test]
fn mapping() {
    let conn = establish_connection();
    let builder = TagsBuilder::new(HashMap::new());
    builder.build(&conn).unwrap();

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
    let econ = Category::create(&conn, "Economics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();
    let store = Category::create(&conn, "Store", 1)
        .and_then(Category::into_leaf)
        .unwrap();
    let book = IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        700, // CNY 700 each
        1,   // Only one book available
        "A very great book on the subject of Economics",
        crate::enums::Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    let subscription = IncompleteProduct::new(
        &store,
        "FLibrary One Subscription (1 month)",
        10, // CNY 10 each
        999,
        "Everything you love in FLibrary at an unbelievable price",
        crate::enums::Currency::CNY,
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    let ads = Tags::find_by_id(&conn, "ads").unwrap();
    let digicon = Tags::find_by_id(&conn, "digicon").unwrap();
    let sales = Tags::find_by_id(&conn, "sales").unwrap();

    TagMapping::create(&conn, &sales, &book).unwrap();
    TagMapping::create(&conn, &ads, &book).unwrap();
    TagMapping::create(&conn, &digicon, &subscription).unwrap();
    TagMapping::create(&conn, &sales, &subscription).unwrap();

    assert_eq!(
        TagMappingFinder::new(&conn, None)
            .product(&book)
            .count()
            .unwrap(),
        2
    );
    assert_eq!(
        TagMappingFinder::new(&conn, None)
            .product(&subscription)
            .count()
            .unwrap(),
        2
    );
}
