#![allow(clippy::unusual_byte_groupings)]
use bitflags::bitflags;
use diesel_derive_enum::DbEnum;
use paypal_rs::data::common::Currency as PayPalCurrency;
use rocket::FromFormField;
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
	// 4 bits in total
	// Access Control on user
        const USER_SELF_READABLE = 0b00_01;
        const USER_SELF_WRITABLE = 0b00_10;
	const USER_OTHERS_READABLE = 0b01_00;
	const USER_OTHERS_WRITABLE = 0b10_00;

	// 6 bits in total
	// Access Control on owned products (user logged in = owner)
	const PROD_SELF_READABLE = 0b000_000_001_0000;
	// changing the products owned and create new products
	const PROD_SELF_WRITABLE = 0b000_000_010_0000;
	const PROD_SELF_REMOVABLE = 0b000_000_100_0000;
	// Access Control on products not owned (user logged in != owner).
	const PROD_OTHERS_READABLE = 0b001_000_0000;
	const PROD_OTHERS_WRITABLE = 0b010_000_0000;
	const PROD_OTHERS_REMOVABLE = 0b100_000_0000;

	// 6 bits in total
	// NOTE: the reading capablity of a digicon for a specific user is in addition determined by the purchase of products containing that digicon.
	// NOTE: modifying digicon mapping is permitted if and only if one can write both products and digicon.
        // Access Control on owned digicons (user logged in = owner)
        const DIGICON_SELF_READABLE = 0b000_001_000000_0000;
	// modify digicons owned and create new digicons
	const DIGICON_SELF_WRITABLE = 0b000_010_000000_0000;
	const DIGICON_SELF_REMOVABLE = 0b000_100_000000_0000;
	// Access Control on digicons not owned (user logged in != owner).
        const DIGICON_OTHERS_READABLE = 0b001_000_000000_0000;
	const DIGICON_OTHERS_WRITABLE = 0b010_000_000000_0000;
	const DIGICON_OTHERS_REMOVABLE = 0b100_000_000000_0000;

	// 12 bits in total
	// NOTE: Progressable also includes the permission to cancel transaction under limited circumstances:
	// 1. Order placed but not paid
	// 2. Order paid but not finished
	// NOTE: Refundable means that the person granted this permission can cancel the order even if it is finished
	// Access Control on transactions in which user acts as the buyer
	const TX_BUYER_READABLE = 0b0000_0000_0001_0000000000000000;
	const TX_BUYER_PROGRESSABLE = 0b0000_0000_0010_0000000000000000;
	const TX_BUYER_FINISHABLE = 0b0000_0000_0100_0000000000000000;
	const TX_BUYER_REFUNDABLE = 0b0000_0000_1000_0000000000000000;
	// Access Control on transactions in which user acts as the seller
	const TX_SELLER_READABLE = 0b0000_0001_0000_0000000000000000;
	const TX_SELLER_PROGRESSABLE = 0b0000_0010_0000_0000000000000000;
	const TX_SELLER_FINISHABLE = 0b0000_0100_0000_0000000000000000;
	const TX_SELLER_REFUNDABLE = 0b0000_1000_0000_0000000000000000;
	// Access Control on transactions in which user acts as a third party
	const TX_OTHERS_READABLE = 0b0001_0000_0000_0000000000000000;
	const TX_OTHERS_PROGRESSABLE = 0b0010_0000_0000_0000000000000000;
	const TX_OTHERS_FINISHABLE = 0b0100_0000_0000_0000000000000000;
	const TX_OTHERS_REFUNDABLE = 0b1000_0000_0000_0000000000000000;

	// Can verify, disable, normalize products;
	const PROD_ADMIN = 0b1_0000_0000_0000_0000000000000000;
	// Can add tag to or remove tag from products;
	const TAG_WRITABLE = 0b10_0000_0000_0000_0000000000000000;

	// Different role profiles
	const DISABLED = 0;
	const NORMAL = Self::USER_SELF_READABLE.bits | Self::USER_SELF_WRITABLE.bits | Self::USER_OTHERS_READABLE.bits
	    | Self::PROD_SELF_READABLE.bits | Self::PROD_OTHERS_READABLE.bits
	    | Self::TX_BUYER_READABLE.bits | Self::TX_BUYER_PROGRESSABLE.bits | Self::TX_SELLER_READABLE.bits;
	// Customer services help users with their orders
	const CUSTOMER_SERVICE = Self::NORMAL.bits | Self::TX_OTHERS_READABLE.bits;
	// Store keepers are responsible for delivering products
	const STORE_KEEPER = Self::CUSTOMER_SERVICE.bits | Self::TX_OTHERS_FINISHABLE.bits;
	// Content creators are allowed to create new products, but they are not able to verify them.
	const CONTENT_CREATOR = Self::NORMAL.bits | Self::PROD_SELF_WRITABLE.bits | Self::PROD_SELF_REMOVABLE.bits | Self::DIGICON_SELF_READABLE.bits | Self::DIGICON_SELF_WRITABLE.bits | Self::DIGICON_SELF_REMOVABLE.bits;
	const ADMIN = 0b11_1111_1111_1111_111111_111111_1111;
    }
}

impl Default for UserStatus {
    fn default() -> Self {
        Self::NORMAL
    }
}

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ProductStatus {
    // The product is currently disabled (not showing up in the store, etc.)
    Disabled,
    // The product is already in warehouse and verified
    Verified,
}

impl Default for ProductStatus {
    fn default() -> Self {
        Self::Disabled
    }
}

impl Status for ProductStatus {
    fn up(&self) -> Self {
        match *self {
            Self::Disabled => Self::Verified,
            Self::Verified => Self::Verified,
        }
    }

    fn down(&self) -> Self {
        match *self {
            Self::Disabled => Self::Disabled,
            Self::Verified => Self::Disabled,
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

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, FromFormField)]
pub enum StorageType {
    // Store files in github release asset
    ReleaseAsset,
    // Store files in github repository
    GitRepo,
    // Store files in S3-compatible object storage
    S3,
}

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, FromFormField)]
pub enum Payment {
    Alipay,
    Paypal,
}

impl Payment {
    pub fn compatible_with(&self, currency: &Currency) -> bool {
        match (self, currency) {
            // AliPay doesn't support multi-currency transaction
            (Self::Alipay, Currency::CNY) => true,
            (Self::Alipay, _) => false,
            // Paypal cannot receive CNY
            (Self::Paypal, Currency::CNY) => false,
            (Self::Paypal, _) => true,
        }
    }
}

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, FromFormField, Hash)]
pub enum Currency {
    /// Chinese renminbi
    CNY,
    /// Euro
    EUR,
    /// Hong Kong dollar
    HKD,
    /// Japanese yen, does not support decimals.
    JPY,
    /// Pound sterling
    GBP,
    /// Swiss franc
    CHF,
    /// United States dollar
    USD,
}

impl From<Currency> for PayPalCurrency {
    fn from(currency: Currency) -> Self {
        match currency {
            Currency::CNY => PayPalCurrency::CNY,
            Currency::EUR => PayPalCurrency::EUR,
            Currency::HKD => PayPalCurrency::HKD,
            Currency::JPY => PayPalCurrency::JPY,
            Currency::GBP => PayPalCurrency::GBP,
            Currency::CHF => PayPalCurrency::CHF,
            Currency::USD => PayPalCurrency::USD,
        }
    }
}

impl From<&Currency> for PayPalCurrency {
    fn from(currency: &Currency) -> Self {
        match currency {
            Currency::CNY => PayPalCurrency::CNY,
            Currency::EUR => PayPalCurrency::EUR,
            Currency::HKD => PayPalCurrency::HKD,
            Currency::JPY => PayPalCurrency::JPY,
            Currency::GBP => PayPalCurrency::GBP,
            Currency::CHF => PayPalCurrency::CHF,
            Currency::USD => PayPalCurrency::USD,
        }
    }
}
