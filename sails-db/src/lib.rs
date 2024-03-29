// False positive by clippy: https://github.com/rust-lang/rust-clippy/issues/9014
#![allow(clippy::extra_unused_lifetimes)]

// For the time being, the diesel doesn't play well without macro use, see also: https://github.com/diesel-rs/diesel/issues/1894
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

pub mod enums;
pub mod error;
pub mod messages;
pub mod products;
#[rustfmt::skip]
mod schema;
pub mod categories;
pub mod coupons;
pub mod digicons;
mod script;
pub mod tags;
pub mod test_utils;
pub mod transactions;
pub mod users;

/// Enum representing order
pub enum Order {
    /// Ascending
    Asc,
    /// Descending
    Desc,
}

/// Enum used for comparison operation
pub enum Cmp {
    /// Greater than
    GreaterThan,
    /// Less than,
    LessThan,
    /// Greater than or equal to,
    GreaterEqual,
    /// Less than or equal to
    LessEqual,
    /// Not equal to
    NotEqual,
    /// Equal to
    Equal,
}
