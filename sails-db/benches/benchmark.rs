use criterion::{criterion_group, criterion_main, Criterion};
use sails_db::{categories::Categories, products::*, test_utils::establish_connection, users::*};

fn login_user(c: &mut Criterion) {
    let conn = establish_connection();
    UserForm::new(
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .to_ref()
    .unwrap()
    .create(&conn)
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
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .to_ref()
    .unwrap()
    .create(&conn)
    .unwrap();

    // The book category
    Categories::create(&conn, "Economics Books").unwrap();
    Categories::create(&conn, "Physics Books").unwrap();

    IncompleteProduct::new(
        "Economics Books",
        "Krugman's Economics 2nd Edition",
        700,
        "A very great book on the subject of Economics",
    )
    .create(&conn, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        "Economics Books",
        "Krugman's Economics 2nd Edition",
        500,
        "A very great book on the subject of Economics",
    )
    .create(&conn, &user_id)
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    IncompleteProduct::new(
        "Economics Books",
        "Krugman's Economics 2nd Edition",
        600,
        "A very great book on the subject of Economics",
    )
    .create(&conn, &user_id)
    .unwrap();

    // Another different economics book
    IncompleteProduct::new(
        "Economics Books",
        "The Economics",
        600,
        "I finally had got a different econ textbook!",
    )
    .create(&conn, &user_id)
    .unwrap();

    // Feynman's Lecture on Physics!
    for _ in 1..200 {
        IncompleteProduct::new(
            "Physics Books",
            "Feynman's Lecture on Physics",
            900,
            "A very masterpiece on the theory of the universe",
        )
        .create(&conn, &user_id)
        .unwrap();
    }

    c.bench_function("search products", |b| {
        b.iter(|| {
            ProductFinder::new(&conn, None)
                .prodname("Krugman's Economics 2nd Edition")
                .price(550, sails_db::Cmp::GreaterThan)
                .search()
                .unwrap()
        })
    });
}

criterion_group!(benches, login_user, products);
criterion_main!(benches);
