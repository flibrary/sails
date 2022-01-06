// Elementary guards for tables in database
mod books;
mod orders;
mod users;
// Role-model that controles the authorization of a specific operation.
mod roles;

pub use self::{books::*, orders::*, roles::*, users::*};
