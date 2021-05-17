use criterion::{criterion_group, criterion_main, Criterion};
use sails_db::{categories::Categories, products::*, test_utils::establish_connection, users::*};

fn login_user(c: &mut Criterion) {
    let conn = establish_connection();
    Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();

    c.bench_function("login an user", |b| {
        b.iter(|| Users::login(&conn, "TestUser@example.org", "strongpasswd").unwrap())
    });
}

fn products(c: &mut Criterion) {
    let conn = establish_connection();
    // our seller
    let user_id = Users::register(
        &conn,
        "TestUser@example.org",
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();

    // The book category
    Categories::create(&conn, "Economics Books").unwrap();
    Categories::create(&conn, "Physics Books").unwrap();

    Products::create(
        &conn,
        user_id.as_str(),
        "Economics Books",
        "Krugman's Economics 2nd Edition",
        700,
        "A very great book on the subject of Economics",
    )
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    Products::create(
        &conn,
        user_id.as_str(),
        "Economics Books",
        "Krugman's Economics 2nd Edition",
        500,
        "A very great book on the subject of Economics",
    )
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    Products::create(
        &conn,
        user_id.as_str(),
        "Economics Books",
        "Krugman's Economics 2nd Edition",
        600,
        "That is a bad book though",
    )
    .unwrap();

    // Another different economics book
    Products::create(
        &conn,
        user_id.as_str(),
        "Economics Books",
        "The Economics",
        600,
        "I finally had got a different econ textbook!",
    )
    .unwrap();

    // Feynman's Lecture on Physics!
    Products::create(
        &conn,
        user_id.as_str(),
        "Physics Books",
        "Feynman's Lecture on Physics",
        900,
        "A very masterpiece on the theory of the universe",
    )
    .unwrap();

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
