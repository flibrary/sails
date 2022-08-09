// Elementary guards for tables in database
mod coupons;
mod digicons;
mod orders;
mod prods;
mod tags;
mod users;
// Role-profile and permission system that controls the authorization of a specific operation.
mod auths;
mod roles;

pub use self::{
    auths::*, coupons::*, digicons::*, orders::*, prods::*, roles::*, tags::*, users::*,
};
