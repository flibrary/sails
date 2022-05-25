use super::{Categories, CtgBuilder, CtgTrait, Value};
use crate::{categories::Category, test_utils::establish_connection};
use uuid::Uuid;

#[test]
fn create_category() {
    let conn = establish_connection();
    let id = Category::create(&conn, "Economics", 1).unwrap();
    // Already created
    assert!(Category::create_with_id(&conn, "Economics", 1, id.id()).is_err());
}

#[test]
fn category_build_and_search() {
    let conn = establish_connection();
    #[rustfmt::skip]
    CtgBuilder::new(maplit::hashmap! {
	"High School".into() => Value::SubCategory{priority: 3, subs: maplit::hashmap!{
    	    "AP".into() => Value::SubCategory{priority: 1, subs: maplit::hashmap!{
		"AP Physics I".into() => Value::Id { id: Uuid::new_v4(), priority: 3 },
		"AP Physics II".into() => Value::Id { id: Uuid::new_v4(), priority: 2 },
		"AP Physics C".into() => Value::Id { id: Uuid::new_v4(), priority: 1 }
	    }},
	    "A Level".into() => Value::SubCategory{priority: 2, subs: maplit::hashmap!{
		"AS Physics".into() => Value::Id { id: Uuid::new_v4(), priority: 2 },
		"A2 Physics".into() => Value::Id { id: Uuid::new_v4(), priority: 1 }
	    }},
	}},
	"University Math".into() => Value::Id { id: Uuid::new_v4(), priority: 1 },
    })
    .build(&conn).unwrap();

    // General testing
    assert_eq!(Categories::list_all(&conn).unwrap().len(), 9);
    assert_eq!(Categories::list_top(&conn).unwrap().len(), 2);

    // Priority should be honored
    assert_eq!(
        Categories::list_leaves::<Category>(&conn, None)
            .unwrap()
            .len(),
        6
    );

    assert_eq!(
        Categories::list_leaves::<Category>(&conn, None).unwrap()[2].name(),
        "AP Physics II"
    );

    // Leaf nodes should have itself upon search
    let ap_phy_2 = Categories::find_by_name(&conn, "AP Physics II").unwrap();

    assert_eq!(
        1,
        Categories::list_leaves(&conn, Some(&ap_phy_2))
            .unwrap()
            .len(),
    );
    assert_eq!(
        ap_phy_2.id(),
        Categories::list_leaves(&conn, Some(&ap_phy_2)).unwrap()[0].id(),
    );

    // Recursive search
    let high_school = Categories::find_by_name(&conn, "High School").unwrap();

    assert_eq!(
        5,
        Categories::list_leaves(&conn, Some(&high_school))
            .unwrap()
            .len(),
    );
}

#[test]
fn manipulate_category() {
    let conn = establish_connection();
    let mut knowledge = Category::create(&conn, "Knowledge", 1).unwrap();
    let mut books = Category::create(&conn, "Books", 1).unwrap();
    let mut economics = Category::create(&conn, "Economics", 1).unwrap();
    let mut physics = Category::create(&conn, "Physics", 1).unwrap();

    // Knowledge, Books -> (Econ, Phys)
    economics.insert(&conn, &mut books).unwrap();
    physics.insert(&conn, &mut books).unwrap();
    assert_eq!(books.subcategory(&conn).unwrap().len(), 2);

    // Knowledge -> Non-electronic -> Books -> (Econ, Phys)
    let mut nonelec = Category::create(&conn, "Non-electronic", 1).unwrap();
    nonelec.insert(&conn, &mut knowledge).unwrap();
    books.insert(&conn, &mut nonelec).unwrap();
    assert_eq!(knowledge.subcategory(&conn).unwrap().len(), 1);
    assert_eq!(nonelec.subcategory(&conn).unwrap().len(), 1);
    assert_eq!(books.subcategory(&conn).unwrap().len(), 2);
    assert_eq!(economics.subcategory(&conn).unwrap().len(), 0);
}
