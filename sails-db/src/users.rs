use crate::{
    enums::UserStatus,
    error::{SailsDbError, SailsDbResult as Result},
    messages::Messages,
    products::Products,
    schema::users,
    Cmp,
};
use diesel::{dsl::count, prelude::*, sqlite::Sqlite};
use rocket::FromForm;
use serde::{Deserialize, Serialize};

/// An user
#[derive(Debug, Serialize, Deserialize, Identifiable, Queryable, Clone)]
#[table_name = "users"]
pub struct UserId {
    id: String,
}

impl UserId {
    pub fn get_info(&self, conn: &SqliteConnection) -> Result<UserInfo> {
        use crate::schema::users::dsl::*;
        Ok(users.filter(id.eq(&self.id)).first::<UserInfo>(conn)?)
    }

    // Quick alternative to user finder
    pub fn find(conn: &SqliteConnection, id: &str) -> Result<Self> {
        UserFinder::new(conn, None).id(id).first()
    }

    pub fn delete(self, conn: &SqliteConnection) -> Result<()> {
        use crate::schema::users::dsl::*;
        Products::delete_by_seller(conn, &self)?;
        Messages::delete_msg_with_user(conn, &self)?;
        diesel::delete(users.filter(id.eq(&self.id))).execute(conn)?;
        Ok(())
    }

    /// Get a reference to the user id's id.
    pub fn get_id(&self) -> &str {
        &self.id
    }
}

type BoxedQuery<'a> = users::BoxedQuery<'a, Sqlite, users::SqlType>;

/// A search query helper (builder)
pub struct UserFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

#[derive(Eq, PartialEq, Debug, Default)]
pub struct UserStats {
    pub total: usize,
    pub disabled: usize,
    pub normal: usize,
    pub customer_service: usize,
    pub store_keeper: usize,
    pub admin: usize,
}

impl<'a> UserFinder<'a> {
    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<UserId>> {
        Self::new(conn, None).search()
    }

    pub fn list_info(conn: &'a SqliteConnection) -> Result<Vec<UserInfo>> {
        Self::new(conn, None).search_info()
    }

    pub fn count(self) -> Result<usize> {
        use crate::schema::users::dsl::*;
        Ok(self.query.select(count(id)).first::<i64>(self.conn)? as usize)
    }

    pub fn stats(conn: &'a SqliteConnection) -> Result<UserStats> {
        // == DISABLED
        let disabled = Self::new(conn, None)
            .status(&UserStatus::DISABLED, Cmp::Equal)
            .count()?;
        // == NORMAL
        let normal = Self::new(conn, None)
            .status(&UserStatus::NORMAL, Cmp::Equal)
            .count()?;
        // CUSTOMER SERVICE <= STATUS < STORE_KEEPER
        let customer_service = Self::new(conn, None)
            .status(&UserStatus::CUSTOMER_SERVICE, Cmp::GreaterEqual)
            .status(&UserStatus::STORE_KEEPER, Cmp::LessThan)
            .count()?;
        let store_keeper = Self::new(conn, None)
            .status(&UserStatus::STORE_KEEPER, Cmp::GreaterEqual)
            .status(&UserStatus::ADMIN, Cmp::LessThan)
            .count()?;
        // == ADMIN
        let admin = Self::new(conn, None)
            .status(&UserStatus::ADMIN, Cmp::Equal)
            .count()?;
        // >= NORMAL
        let total = Self::new(conn, None)
            .status(&UserStatus::NORMAL, Cmp::GreaterEqual)
            .count()?;

        Ok(UserStats {
            total,
            disabled,
            normal,
            admin,
            customer_service,
            store_keeper,
        })
    }

    pub fn new(conn: &'a SqliteConnection, query: Option<BoxedQuery<'a>>) -> Self {
        use crate::schema::users::dsl::*;
        if let Some(q) = query {
            Self { conn, query: q }
        } else {
            Self {
                conn,
                query: users.into_boxed(),
            }
        }
    }

    pub fn first(self) -> Result<UserId> {
        use crate::schema::users::dsl::*;
        Ok(UserId {
            id: self.query.select(id).first::<String>(self.conn)?,
        })
    }

    pub fn first_info(self) -> Result<UserInfo> {
        Ok(self.query.first::<UserInfo>(self.conn)?)
    }

    pub fn search(self) -> Result<Vec<UserId>> {
        use crate::schema::users::dsl::*;
        Ok(self
            .query
            .select(id)
            .load::<String>(self.conn)?
            .into_iter()
            .map(|x| UserId { id: x })
            .collect())
    }

    pub fn search_info(self) -> Result<Vec<UserInfo>> {
        Ok(self.query.load::<UserInfo>(self.conn)?)
    }

    pub fn id(mut self, id_provided: &'a str) -> Self {
        use crate::schema::users::dsl::*;
        self.query = self.query.filter(id.eq(id_provided));
        self
    }

    pub fn school(mut self, school_provided: &'a str) -> Self {
        use crate::schema::users::dsl::*;
        self.query = self.query.filter(school.eq(school_provided));
        self
    }

    pub fn name(mut self, name_provided: &'a str) -> Self {
        use crate::schema::users::dsl::*;
        self.query = self.query.filter(name.eq(name_provided));
        self
    }

    pub fn status(mut self, status: &'a UserStatus, cmp: Cmp) -> Self {
        use crate::schema::users::dsl::*;
        match cmp {
            Cmp::Equal => self.query = self.query.filter(user_status.eq(status.bits() as i64)),
            Cmp::NotEqual => self.query = self.query.filter(user_status.ne(status.bits() as i64)),
            Cmp::GreaterThan => {
                self.query = self.query.filter(user_status.gt(status.bits() as i64))
            }
            Cmp::LessThan => self.query = self.query.filter(user_status.lt(status.bits() as i64)),
            Cmp::GreaterEqual => {
                self.query = self.query.filter(user_status.ge(status.bits() as i64))
            }
            Cmp::LessEqual => self.query = self.query.filter(user_status.le(status.bits() as i64)),
        }
        self
    }

    pub fn allowed(mut self) -> Self {
        use crate::schema::users::dsl::*;
        self.query = self
            .query
            .filter(user_status.ne(UserStatus::DISABLED.bits() as i64));
        self
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, AsChangeset, Identifiable, Clone)]
#[table_name = "users"]
pub struct UserInfo {
    id: String,
    name: String,
    school: String,
    description: Option<String>,
    user_status: i64,
}

impl UserInfo {
    pub fn to_id(&self) -> UserId {
        UserId {
            id: self.id.to_string(),
        }
    }

    /// Get a reference to the user info's id.
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Get a reference to the user info's school.
    pub fn get_school(&self) -> &str {
        &self.school
    }

    /// Set the user info's school.
    pub fn set_school(mut self, school: impl ToString) -> Self {
        self.school = school.to_string();
        self
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<UserInfo>(conn)?)
    }

    /// Get a reference to the user info's name.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set the user info's name.
    pub fn set_name(mut self, name: impl ToString) -> Self {
        self.name = name.to_string();
        self
    }

    /// Get a reference to the user info's user status.
    pub fn get_user_status(&self) -> UserStatus {
        UserStatus::from_bits_truncate(self.user_status as u32)
    }

    /// Set the user info's user status.
    pub fn set_user_status(mut self, user_status: UserStatus) -> Self {
        self.user_status = user_status.bits() as i64;
        self
    }

    /// See if the user is admin or not
    pub fn is_admin(&self) -> bool {
        UserStatus::from_bits_truncate(self.user_status as u32).contains(UserStatus::ADMIN)
    }

    /// Get a reference to the user info's description.
    pub fn get_description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Set the user info's description.
    pub fn set_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }
}

// A struct used for update and insert
#[derive(Debug, Serialize, Deserialize, Insertable, AsChangeset, Identifiable, Clone)]
#[table_name = "users"]
pub struct UserInfoRef<'a> {
    id: &'a str,
    name: &'a str,
    school: &'a str,
    description: Option<&'a str>,
    // This is owned because it was created when convert to UserInfoRef
    user_status: i64,
}

impl<'a> UserInfoRef<'a> {
    pub fn create(self, conn: &SqliteConnection) -> Result<UserId> {
        use crate::schema::users::dsl::*;
        let id_cloned = self.id.to_string();
        if let Ok(0) = users.filter(id.eq(self.id)).count().get_result(conn) {
            // This means that there is no user existed before
            diesel::insert_into(users).values(self).execute(conn)?
        } else {
            return Err(SailsDbError::UserRegistered);
        };
        Ok(UserId { id: id_cloned })
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<UserInfo> {
        Ok(self.save_changes::<UserInfo>(conn)?)
    }
}

// This can be created by rocket and can be converted into insertable user
#[derive(Debug, Serialize, Deserialize, FromForm, Clone)]
pub struct UserFormOwned {
    #[field(name = "email")]
    pub id: String,
    pub name: String,
    pub school: String,
    pub description: Option<String>,
}

impl UserFormOwned {
    pub fn new<T: ToString>(id: T, name: T, school: T, description: Option<T>) -> Self {
        Self {
            id: id.to_string(),
            school: school.to_string(),
            name: name.to_string(),
            description: description.map(|x| x.to_string()),
        }
    }

    pub fn to_ref(&self) -> Result<UserInfoRef> {
        let form = UserForm {
            id: &self.id,
            school: &self.school,
            name: &self.name,
            description: self.description.as_deref(),
        };
        form.to_ref()
    }
}

// This can be created by rocket and can be converted into insertable user
#[derive(Debug, Serialize, Deserialize, FromForm, Clone)]
pub struct UserForm<'a> {
    #[field(name = "email")]
    pub id: &'a str,
    pub name: &'a str,
    pub school: &'a str,
    pub description: Option<&'a str>,
}

impl<'a> UserForm<'a> {
    pub fn new(id: &'a str, name: &'a str, school: &'a str, description: Option<&'a str>) -> Self {
        Self {
            id,
            name,
            school,
            description,
        }
    }

    // Warning: this should not be used to update user!
    // Otherwise the account role gets cleaned up to default.
    pub fn to_ref(&self) -> Result<UserInfoRef<'a>> {
        self.id.parse::<lettre::Address>()?;
        Ok(UserInfoRef {
            id: self.id,
            school: self.school,
            name: self.name,
            description: self.description,
            user_status: UserStatus::default().bits() as i64,
        })
    }
}

#[cfg(test)]
mod tests;
