use criterion::{criterion_group, criterion_main, Criterion};
use sails_db::{
    categories::{Category, CtgTrait},
    products::*,
    test_utils::establish_connection,
    users::*,
};

fn login_user(c: &mut Criterion) {
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

    user_id
        .get_info(&conn)
        .unwrap()
        .set_validated(true)
        .update(&conn)
        .unwrap();

    c.bench_function("login an user", |b| {
        b.iter(|| UserId::login(&conn, "TestUser@example.org", "strongpasswd").unwrap())
    });
}

fn products(c: &mut Criterion) {
    let conn = establish_connection();
    // our seller
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

    // The book category
    let mut books = Category::create(&conn, "Books", 1).unwrap();
    let mut econ = Category::create(&conn, "Economics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();
    let mut phys = Category::create(&conn, "Physics Books", 1)
        .and_then(Category::into_leaf)
        .unwrap();

    econ.insert(&conn, &mut books).unwrap();
    phys.insert(&conn, &mut books).unwrap();

    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        700,
        4,
        "A very great book on the subject of Economics",
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        500,
        3,
        "A very great book on the subject of Economics",
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        &econ,
        "Krugman's Economics 2nd Edition",
        600,
        3,
        "A very great book on the subject of Economics",
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Another different economics book
    IncompleteProduct::new(
        &econ,
        "The Economics",
        600,
        1,
        "I finally had got a different econ textbook!",
    )
    .unwrap()
    .create(&conn, &user_id)
    .unwrap();

    // Feynman's Lecture on Physics!
    for _ in 1..200 {
        IncompleteProduct::new(
            &phys,
            "Feynman's Lecture on Physics",
            900,
            100,
            "A very masterpiece on the theory of the universe",
        )
        .unwrap()
        .create(&conn, &user_id)
        .unwrap();
    }

    c.bench_function("search products by price", |b| {
        b.iter(|| {
            ProductFinder::new(&conn, None)
                .prodname("Krugman's Economics 2nd Edition")
                .price(550, sails_db::Cmp::LessThan)
                .search()
                .unwrap()
        })
    });

    c.bench_function("search products by category", |b| {
        b.iter(|| {
            ProductFinder::new(&conn, None)
                .category(&books)
                .unwrap()
                .price(550, sails_db::Cmp::LessThan)
                .search()
                .unwrap()
        })
    });
}

criterion_group!(benches, login_user, products);
criterion_main!(benches);
