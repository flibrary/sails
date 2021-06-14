use super::{Categories, CtgBuilder, CtgTrait, Value};
use crate::{categories::Category, test_utils::establish_connection};
use uuid::Uuid;

#[test]
fn create_category() {
    let conn = establish_connection();
    let id = Category::create(&conn, "Economics").unwrap();
    // Already created
    assert!(Category::create_with_id(&conn, "Economics", id.id()).is_err());
}

#[test]
fn category_builder() {
    let conn = establish_connection();
    #[rustfmt::skip]
    CtgBuilder::new(maplit::btreemap! {
	"AP".into() => Value::SubCategory(maplit::btreemap!{
            "AP Physics I".into() => Value::Id(Uuid::new_v4()),
            "AP Physics II".into() => Value::Id(Uuid::new_v4()),
            "AP Physics C".into() => Value::Id(Uuid::new_v4())
	}),
	"A Level".into() => Value::SubCategory(maplit::btreemap!{
            "AS Physics".into() => Value::Id(Uuid::new_v4()),
	    "A2 Physics".into() => Value::Id(Uuid::new_v4())
	}),
	"University Math".into() => Value::Id(Uuid::new_v4()),
    })
    .build(&conn).unwrap();

    assert_eq!(Categories::list_all(&conn).unwrap().len(), 8);
}

#[test]
fn manipulate_category() {
    let conn = establish_connection();
    let mut knowledge = Category::create(&conn, "Knowledge").unwrap();
    let mut books = Category::create(&conn, "Books").unwrap();
    let mut economics = Category::create(&conn, "Economics").unwrap();
    let mut physics = Category::create(&conn, "Physics").unwrap();

    // Knowledge, Books -> (Econ, Phys)
    economics.insert(&conn, &mut books).unwrap();
    physics.insert(&conn, &mut books).unwrap();
    assert_eq!(books.subcategory(&conn).unwrap().len(), 2);

    // Knowledge -> Non-electronic -> Books -> (Econ, Phys)
    let mut nonelec = Category::create(&conn, "Non-electronic").unwrap();
    nonelec.insert(&conn, &mut knowledge).unwrap();
    books.insert(&conn, &mut nonelec).unwrap();
    assert_eq!(knowledge.subcategory(&conn).unwrap().len(), 1);
    assert_eq!(nonelec.subcategory(&conn).unwrap().len(), 1);
    assert_eq!(books.subcategory(&conn).unwrap().len(), 2);
    assert_eq!(economics.subcategory(&conn).unwrap().len(), 0);
}
