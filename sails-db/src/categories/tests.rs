use super::Categories;
use crate::test_utils::establish_connection;

#[test]
fn create_category() {
    let conn = establish_connection();
    let id = Categories::create(&conn, "Economics").unwrap();
    // Already created
    assert!(Categories::create_with_id(&conn, "Economics", id).is_err());
}

#[test]
fn manipulate_category() {
    let conn = establish_connection();
    let knowledge = Categories::create(&conn, "Knowledge").unwrap();
    let books = Categories::create(&conn, "Books").unwrap();
    let economics = Categories::create(&conn, "Economics").unwrap();
    let physics = Categories::create(&conn, "Physics").unwrap();

    // Knowledge, Books -> (Econ, Phys)
    Categories::insert(&conn, &economics, &books).unwrap();
    Categories::insert(&conn, &physics, &books).unwrap();
    assert_eq!(Categories::subcategory(&conn, &books).unwrap().len(), 2);

    // Knowledge -> Non-electronic -> Books -> (Econ, Phys)
    let nonelec = Categories::create(&conn, "Non-electronic").unwrap();
    Categories::insert(&conn, &nonelec, &knowledge).unwrap();
    Categories::insert(&conn, &books, &nonelec).unwrap();
    assert_eq!(Categories::subcategory(&conn, &knowledge).unwrap().len(), 1);
    assert_eq!(Categories::subcategory(&conn, &nonelec).unwrap().len(), 1);
    assert_eq!(Categories::subcategory(&conn, &books).unwrap().len(), 2);
    assert_eq!(Categories::subcategory(&conn, &economics).unwrap().len(), 0);
}
