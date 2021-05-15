// For the time being, the diesel doesn't play well without macro use, see also: https://github.com/diesel-rs/diesel/issues/1894
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

pub mod error;
pub mod products;
#[rustfmt::skip]
mod schema;
pub mod categories;
pub mod test_utils;
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
}
