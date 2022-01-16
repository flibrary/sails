use super::{Categories, CtgBuilder, CtgTrait, Value};
use crate::{categories::Category, test_utils::establish_connection};
use uuid::Uuid;

#[test]
fn create_category() {
    let conn = establish_connection();
    let id = Category::create(&conn, "Economics", 490).unwrap();
    // Already created
    assert!(Category::create_with_id(&conn, "Economics", 490, id.id()).is_err());
}

#[test]
fn category_builder() {
    let conn = establish_connection();
    #[rustfmt::skip]
    CtgBuilder::new(maplit::btreemap! {
	"AP".into() => Value::SubCategory(maplit::btreemap!{
            "AP Physics I".into() => Value::Id { id: Uuid::new_v4(), price: 630 },
            "AP Physics II".into() => Value::Id { id: Uuid::new_v4(), price: 630 },
            "AP Physics C".into() => Value::Id { id: Uuid::new_v4(), price: 1630 }
	}),
	"A Level".into() => Value::SubCategory(maplit::btreemap!{
            "AS Physics".into() => Value::Id { id: Uuid::new_v4(), price: 300 },
	    "A2 Physics".into() => Value::Id { id: Uuid::new_v4(), price: 100 }
	}),
	"University Math".into() => Value::Id { id: Uuid::new_v4(), price: 2000 },
    })
    .build(&conn).unwrap();

    assert_eq!(Categories::list_all(&conn).unwrap().len(), 8);
    assert_eq!(Categories::list_top(&conn).unwrap().len(), 3);
}

#[test]
fn manipulate_category() {
    let conn = establish_connection();
    let mut knowledge = Category::create(&conn, "Knowledge", 0).unwrap();
    let mut books = Category::create(&conn, "Books", 0).unwrap();
    let mut economics = Category::create(&conn, "Economics", 300).unwrap();
    let mut physics = Category::create(&conn, "Physics", 300).unwrap();

    // Knowledge, Books -> (Econ, Phys)
    economics.insert(&conn, &mut books).unwrap();
    physics.insert(&conn, &mut books).unwrap();
    assert_eq!(books.subcategory(&conn).unwrap().len(), 2);

    // Knowledge -> Non-electronic -> Books -> (Econ, Phys)
    let mut nonelec = Category::create(&conn, "Non-electronic", 0).unwrap();
    nonelec.insert(&conn, &mut knowledge).unwrap();
    books.insert(&conn, &mut nonelec).unwrap();
    assert_eq!(knowledge.subcategory(&conn).unwrap().len(), 1);
    assert_eq!(nonelec.subcategory(&conn).unwrap().len(), 1);
    assert_eq!(books.subcategory(&conn).unwrap().len(), 2);
    assert_eq!(economics.subcategory(&conn).unwrap().len(), 0);
}
