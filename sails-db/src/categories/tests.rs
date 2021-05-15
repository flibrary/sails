use super::Categories;
use crate::test_utils::establish_connection;

#[test]
fn create_category() {
    let conn = establish_connection();
    Categories::create(&conn, "Economics").unwrap();
}

#[test]
fn manipulate_category() {
    let conn = establish_connection();
    let knowledge_id = Categories::create(&conn, "Knowledge").unwrap();
    let book_id = Categories::create(&conn, "Books").unwrap();
    let econ_id = Categories::create(&conn, "Economics").unwrap();
    let phys_id = Categories::create(&conn, "Physics").unwrap();

    // Knowledge, Books -> (Econ, Phys)
    Categories::insert(&conn, &econ_id, &book_id).unwrap();
    Categories::insert(&conn, &phys_id, &book_id).unwrap();
    assert_eq!(Categories::subcategory(&conn, &book_id).unwrap().len(), 2);

    // Knowledge -> Non-electronic -> Books -> (Econ, Phys)
    let non_elec_id = Categories::create(&conn, "Non-electronic").unwrap();
    Categories::insert(&conn, &non_elec_id, &knowledge_id).unwrap();
    Categories::insert(&conn, &book_id, &non_elec_id).unwrap();
    assert_eq!(
        Categories::subcategory(&conn, &knowledge_id).unwrap().len(),
        1
    );
    assert_eq!(
        Categories::subcategory(&conn, &non_elec_id).unwrap().len(),
        1
    );
    assert_eq!(Categories::subcategory(&conn, &book_id).unwrap().len(), 2);
    assert_eq!(Categories::subcategory(&conn, &econ_id).unwrap().len(), 0);
}
