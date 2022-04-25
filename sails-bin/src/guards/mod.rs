// Elementary guards for tables in database
mod orders;
mod prods;
mod tags;
mod users;
// Role-profile and permission system that controls the authorization of a specific operation.
mod auths;
mod roles;

pub use self::{auths::*, orders::*, prods::*, roles::*, tags::*, users::*};
