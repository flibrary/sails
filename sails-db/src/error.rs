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

    #[error("invalid phone number or email address")]
    InvalidIdentity,

    #[error("email has already been registered")]
    UserRegistered,

    #[error("no user found given the information")]
    UserNotFound,

    #[error("password was incorrect")]
    IncorrectPassword,

    #[error("category already existed")]
    CategoryExisted,

    #[error("category doesn't exist")]
    CategoryNotFound,

    #[error("product doesn't exist")]
    ProductNotFound,

    #[error("non-leaf category is not allowed for the request")]
    NonLeafCategory,

    #[error("the user has been disabled")]
    DisabledUser,
}
