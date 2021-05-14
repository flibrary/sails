// For the time being, the diesel doesn't play well without macro use, see also: https://github.com/diesel-rs/diesel/issues/1894
#[macro_use]
extern crate diesel;
#[cfg(test)]
#[macro_use]
extern crate diesel_migrations;

pub mod error;
pub mod products;
#[rustfmt::skip]
mod schema;
#[cfg(test)]
mod test_utils;
pub mod users;
