use std::num::NonZeroI64;

use criterion::{criterion_group, criterion_main, Criterion};
use sails_db::{products::*, test_utils::establish_connection, users::*};

fn login_user(c: &mut Criterion) {
    let conn = establish_connection();
    Users::register(
        &conn,
        "TestUser",
        None,
        "NFLS",
        "+86 18353232340",
        "strongpasswd",
    )
    .unwrap();

    c.bench_function("login an user", |b| {
        b.iter(|| Users::login(&conn, "TestUser", "strongpasswd").unwrap())
    });
}

fn products(c: &mut Criterion) {
    let conn = establish_connection();
    // our seller
    let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
    Users::create_or_update(&conn, user.clone()).unwrap();
    Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(700).unwrap(),
        "A very great book on the subject of Economics",
    )
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(500).unwrap(),
        "A very great book on the subject of Economics",
    )
    .unwrap();

    // Another Krugman's Economics, with a lower price!
    Products::create_product(
        &conn,
        &user,
        "Krugman's Economics 2nd Edition",
        NonZeroI64::new(600).unwrap(),
        "That is a bad book though",
    )
    .unwrap();

    // Feynman's Lecture on Physics!
    Products::create_product(
        &conn,
        &user,
        "Feynman's Lecture on Physics",
        NonZeroI64::new(900).unwrap(),
        "A very masterpiece on the theory of the universe",
    )
    .unwrap();

    c.bench_function("search products", |b| {
        b.iter(|| {
            ProductFinder::new(&conn, None)
                .prodname("Krugman's Economics 2nd Edition")
                .price(NonZeroI64::new(550).unwrap(), sails_db::Cmp::GreaterThan)
                .search()
                .unwrap()
        })
    });
}

criterion_group!(benches, login_user, products);
criterion_main!(benches);
