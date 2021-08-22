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

impl Default for ProductStatus {
    fn default() -> Self {
        Self::Normal
    }
}

impl Status for ProductStatus {
    fn up(&self) -> Self {
        match *self {
            Self::Disabled => Self::Normal,
            Self::Normal => Self::Verified,
            Self::Verified => Self::Sold,
            Self::Sold => Self::Sold,
        }
    }

    fn down(&self) -> Self {
        match *self {
            Self::Disabled => Self::Disabled,
            Self::Normal => Self::Disabled,
            Self::Verified => Self::Normal,
            Self::Sold => Self::Verified,
        }
    }
}

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransactionStatus {
    // The product has already been sold
    Refunded,
    // The order/transaction has been placed
    Placed,
    // Buyer has paid the price
    Paid,
    // The transaction has been finished (product is delivered and price has been paid)
    Finished,
}

impl Default for TransactionStatus {
    fn default() -> Self {
        Self::Placed
    }
}

impl Status for TransactionStatus {
    fn up(&self) -> Self {
        match *self {
            Self::Refunded => Self::Placed,
            Self::Placed => Self::Paid,
            Self::Paid => Self::Finished,
            Self::Finished => Self::Finished,
        }
    }

    fn down(&self) -> Self {
        match *self {
            Self::Refunded => Self::Refunded,
            Self::Placed => Self::Refunded,
            Self::Paid => Self::Placed,
            Self::Finished => Self::Paid,
        }
    }
}
