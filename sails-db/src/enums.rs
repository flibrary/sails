#![allow(clippy::unusual_byte_groupings)]
use bitflags::bitflags;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};

// A trait that defines the status of a user / product, this typically can progress
pub trait Status: Default {
    // Upgrade to the next status (no wrapping)
    fn up(&self) -> Self;
    // Downgrade to a lower status (no wrapping)
    fn down(&self) -> Self;
}

#[rustfmt::skip]
bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct UserStatus: u32 {
	// Access Control on user
        const USER_SELF_READABLE = 0b00_01;
        const USER_SELF_WRITABLE = 0b00_10;
	const USER_OTHERS_READABLE = 0b01_00;
	const USER_OTHERS_WRITABLE = 0b10_00;

	// Access Control on owned products (user logged in = operator = owner)
	const PROD_SELF_READABLE = 0b000_000_001_0000;
	const PROD_SELF_WRITABLE = 0b000_000_010_0000;
	const PROD_SELF_REMOVABLE = 0b000_000_100_0000;
	// Access Control on delegated products (user logged in = operator != owner)
	const PROD_DELG_READABLE = 0b000_001_000_0000;
	const PROD_DELG_WRITABLE = 0b000_010_000_0000;
	const PROD_DELG_REMOVABLE = 0b000_100_000_0000;
	// Access Control on products neither owned nor delegated (user logged in != operator).
	// user = owner != operator falls into this category
	const PROD_OTHERS_READABLE = 0b001_000000_0000;
	const PROD_OTHERS_WRITABLE = 0b010_000000_0000;
	const PROD_OTHERS_REMOVABLE = 0b100_000000_0000;

	// NOTE: Progressable also includes the permission to cancel transaction under limited circumstances:
	// 1. Order placed but not paid
	// 2. Order paid but not finished
	// NOTE: Refundable means that the person granted this permission can cancel the order even if it is finished
	// Access Control on transactions in which user acts as the buyer
	const TX_BUYER_READABLE = 0b0000_0000_0001_0000000000000;
	const TX_BUYER_PROGRESSABLE = 0b0000_0000_0010_0000000000000;
	const TX_BUYER_FINISHABLE = 0b0000_0000_0100_0000000000000;
	const TX_BUYER_REFUNDABLE = 0b0000_0000_1000_0000000000000;
	// Access Control on transactions in which user acts as the seller
	const TX_SELLER_READABLE = 0b0000_0001_0000_0000000000000;
	const TX_SELLER_PROGRESSABLE = 0b0000_0010_0000_0000000000000;
	const TX_SELLER_FINISHABLE = 0b0000_0100_0000_0000000000000;
	const TX_SELLER_REFUNDABLE = 0b0000_1000_0000_0000000000000;
	// Access Control on transactions in which user acts as a third party
	const TX_OTHERS_READABLE = 0b0001_0000_0000_0000000000000;
	const TX_OTHERS_PROGRESSABLE = 0b0010_0000_0000_0000000000000;
	const TX_OTHERS_FINISHABLE = 0b0100_0000_0000_0000000000000;
	const TX_OTHERS_REFUNDABLE = 0b1000_0000_0000_0000000000000;

	// Can verify, disable, normalize products;
	const PROD_ADMIN = 0b1_0000_0000_0000_0000000000000;

	// Different role profiles
	const DISABLED = 0;
	const NORMAL = Self::USER_SELF_READABLE.bits | Self::USER_SELF_WRITABLE.bits | Self::USER_OTHERS_READABLE.bits
	    | Self::PROD_SELF_READABLE.bits | Self::PROD_SELF_WRITABLE.bits | Self::PROD_SELF_REMOVABLE.bits | Self::PROD_DELG_READABLE.bits | Self::PROD_DELG_WRITABLE.bits | Self::PROD_OTHERS_READABLE.bits
	    | Self::TX_BUYER_READABLE.bits | Self::TX_BUYER_PROGRESSABLE.bits | Self::TX_SELLER_READABLE.bits;
	const CUSTOMER_SERVICE = Self::NORMAL.bits | Self::TX_OTHERS_READABLE.bits;
	const STORE_KEEPER = Self::CUSTOMER_SERVICE.bits | Self::TX_OTHERS_FINISHABLE.bits | Self::PROD_ADMIN.bits;
	const ADMIN = 0b1_1111_1111_1111_111111111_1111;
    }
}

impl Default for UserStatus {
    fn default() -> Self {
        Self::NORMAL
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
