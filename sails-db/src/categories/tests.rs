use super::Categories;
use crate::test_utils::establish_connection;

#[test]
fn create_category() {
    let conn = establish_connection();
    Categories::create(&conn, "Economics").unwrap();
    // Already created
    assert!(Categories::create(&conn, "Economics").is_err());
}

#[test]
fn manipulate_category() {
    let conn = establish_connection();
    Categories::create(&conn, "Knowledge").unwrap();
    Categories::create(&conn, "Books").unwrap();
    Categories::create(&conn, "Economics").unwrap();
    Categories::create(&conn, "Physics").unwrap();

    // Knowledge, Books -> (Econ, Phys)
    Categories::insert(&conn, "Economics", "Books").unwrap();
    Categories::insert(&conn, "Physics", "Books").unwrap();
    assert_eq!(Categories::subcategory(&conn, "Books").unwrap().len(), 2);

    // Knowledge -> Non-electronic -> Books -> (Econ, Phys)
    Categories::create(&conn, "Non-electronic").unwrap();
    Categories::insert(&conn, "Non-electronic", "Knowledge").unwrap();
    Categories::insert(&conn, "Books", "Non-electronic").unwrap();
    assert_eq!(
        Categories::subcategory(&conn, "Knowledge").unwrap().len(),
        1
    );
    assert_eq!(
        Categories::subcategory(&conn, "Non-electronic")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(Categories::subcategory(&conn, "Books").unwrap().len(), 2);
    assert_eq!(
        Categories::subcategory(&conn, "Economics").unwrap().len(),
        0
    );
}
