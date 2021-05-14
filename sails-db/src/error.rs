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

    #[error("invalid phone number")]
    InvalidPhoneNumber,

    #[error("username already exists")]
    UserRegistered,
}
