use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};

// A trait that defines the status of a user / product, this typically can progress
pub trait Status: Default {
    // Upgrade to the next status (no wrapping)
    fn up(&self) -> Self;
    // Downgrade to a lower status (no wrapping)
    fn down(&self) -> Self;
}

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserStatus {
    Normal,
    Admin,
    Disabled,
}

impl Default for UserStatus {
    fn default() -> Self {
        Self::Normal
    }
}

impl Status for UserStatus {
    fn up(&self) -> Self {
        match *self {
            Self::Disabled => Self::Normal,
            Self::Normal => Self::Admin,
            Self::Admin => Self::Admin,
        }
    }

    fn down(&self) -> Self {
        match *self {
            Self::Disabled => Self::Disabled,
            Self::Normal => Self::Disabled,
            Self::Admin => Self::Normal,
        }
    }
}

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProductStatus {
    // The product has already been sold
    Sold,
    Normal,
    // The product is already in warehouse and verified
    Verified,
    // The product is considered harzard, and it has been disabled
    Disabled,
}
