use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};

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
