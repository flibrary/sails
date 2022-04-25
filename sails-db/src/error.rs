use bcrypt::BcryptError;
use thiserror::Error;

pub type SailsDbResult<T> = Result<T, SailsDbError>;

#[derive(Error, Debug)]
pub enum SailsDbError {
    #[error("errored on hashing using bcrypt: {0}")]
    HashError(#[from] BcryptError),

    #[error("database query failed with: {0}")]
    QueryError(#[from] diesel::result::Error),

    #[error("failed to convert to a phone number: {0}")]
    PhoneParseError(#[from] phonenumber::ParseError),

    #[error("failed to parse uuid: {0}")]
    UuidError(#[from] uuid::Error),

    #[error("invalid email address: {0}")]
    InvalidIdentity(#[from] lettre::address::AddressError),

    #[error("email has already been registered")]
    UserRegistered,

    #[error("no user found given the information")]
    UserNotFound,

    #[error("password was incorrect")]
    IncorrectPassword,

    #[error("category already existed")]
    CategoryExisted,

    #[error("tag already existed")]
    TagExisted,

    #[error("tag mapping already existed")]
    TagMappingExisted,

    #[error("digicon already existed")]
    DigiconExisted,

    #[error("digicon mapping already existed")]
    DigiconMappingExisted,

    #[error("category doesn't exist")]
    CategoryNotFound,

    #[error("product doesn't exist")]
    ProductNotFound,

    #[error("non-leaf category is not allowed for the request")]
    NonLeafCategory,

    #[error("the product has already been sold out")]
    ProductSoldOut,

    #[error("illegal price or quantity provided")]
    IllegalPriceOrQuantity,

    #[error("internal overflow")]
    Overflow,

    #[error("purchasing unverified products")]
    OrderOnUnverified,

    #[error("seller cannot purchase his/her own products")]
    SelfPurchaseNotAllowed,

    #[error("failed to change available product quantity on purchase")]
    FailedAlterProductQuantity,

    #[error("the user has been disabled")]
    DisabledUser,

    #[error("the user's was not verified. Please check your mailbox and junk folder to verify.")]
    NotValidatedEmail,

    #[error("illegal query")]
    IllegalQuery,

    #[error("other errors: {0}")]
    Anyhow(#[from] anyhow::Error),
}
