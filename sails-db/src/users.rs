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

    pub fn delete(self, conn: &SqliteConnection) -> Result<()> {
        use crate::schema::users::dsl::*;
        Products::delete_by_seller(conn, &self)?;
        Messages::delete_msg_with_user(conn, &self)?;
        diesel::delete(users.filter(id.eq(&self.id))).execute(conn)?;
        Ok(())
    }

    pub fn login(
        conn: &SqliteConnection,
        id_provided: &str,
        passwd_provided: &str,
    ) -> Result<Self> {
        let user = UserFinder::new(conn, None)
            .id(id_provided)
            .first()
            .map_err(|_| SailsDbError::UserNotFound)?;
        let info = user.get_info(conn)?;

        // If the user is disabled, he will not be allowed to login
        if info.get_user_status() != UserStatus::DISABLED {
            if info.get_validated() {
                match info.verify_passwd(passwd_provided) {
                    Ok(true) => {
                        // Successfully validated
                        Ok(user)
                    }
                    Ok(false) => {
                        // User exists, but password is not right
                        Err(SailsDbError::IncorrectPassword)
                    }
                    Err(e) => Err(e),
                }
            } else {
                Err(SailsDbError::NotValidatedEmail)
            }
        } else {
            Err(SailsDbError::DisabledUser)
        }
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
        let disabled = Self::new(conn, None)
            .status(&UserStatus::DISABLED, Cmp::Equal)
            .count()?;
        let normal = Self::new(conn, None)
            .status(&UserStatus::NORMAL, Cmp::Equal)
            .count()?;
        let admin = Self::new(conn, None)
            .status(&UserStatus::ADMIN, Cmp::Equal)
            .count()?;

        Ok(UserStats {
            total: normal + admin,
            disabled,
            normal,
            admin,
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
            // Currently it makes no sense for us to do so
            _ => unimplemented!(),
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

    pub fn validated(mut self, val: bool) -> Self {
        use crate::schema::users::dsl::*;
        self.query = self.query.filter(validated.eq(val));
        self
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, AsChangeset, Identifiable, Clone)]
#[table_name = "users"]
pub struct UserInfo {
    id: String,
    name: String,
    school: String,
    hashed_passwd: String,
    validated: bool,
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

    pub fn verify_passwd(&self, passwd: impl AsRef<[u8]>) -> Result<bool> {
        Ok(bcrypt::verify(passwd, &self.hashed_passwd)?)
    }

    /// Set the user info's school.
    pub fn set_school(mut self, school: impl ToString) -> Self {
        self.school = school.to_string();
        self
    }

    pub fn set_password(mut self, raw_passwd: impl AsRef<[u8]>) -> Result<Self> {
        self.hashed_passwd = bcrypt::hash(raw_passwd, bcrypt::DEFAULT_COST)?;
        Ok(self)
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

    /// Get a reference to the user info's validated.
    pub fn get_validated(&self) -> bool {
        self.validated
    }

    /// Set the user info's validated.
    pub fn set_validated(mut self, validated: bool) -> Self {
        self.validated = validated;
        self
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
    // This is owned because we processed it
    hashed_passwd: String,
    validated: bool,
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
    #[field(name = "password")]
    pub raw_passwd: String,
}

impl UserFormOwned {
    pub fn new<T: ToString>(
        id: T,
        name: T,
        school: T,
        raw_passwd: T,
        description: Option<T>,
    ) -> Self {
        Self {
            id: id.to_string(),
            school: school.to_string(),
            name: name.to_string(),
            raw_passwd: raw_passwd.to_string(),
            description: description.map(|x| x.to_string()),
        }
    }

    pub fn to_ref(&self) -> Result<UserInfoRef> {
        let form = UserForm {
            id: &self.id,
            school: &self.school,
            name: &self.name,
            description: self.description.as_deref(),
            raw_passwd: &self.raw_passwd,
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
    #[field(name = "password")]
    pub raw_passwd: &'a str,
}

impl<'a> UserForm<'a> {
    pub fn new(
        id: &'a str,
        name: &'a str,
        school: &'a str,
        raw_passwd: &'a str,
        description: Option<&'a str>,
    ) -> Self {
        Self {
            id,
            name,
            school,
            description,
            raw_passwd,
        }
    }

    // Warning: this should not be used to update user!
    // Otherwise the validation and account role gets cleaned up to default.
    pub fn to_ref(&self) -> Result<UserInfoRef<'a>> {
        self.id.parse::<lettre::Address>()?;
        Ok(UserInfoRef {
            id: self.id,
            hashed_passwd: bcrypt::hash(self.raw_passwd, bcrypt::DEFAULT_COST)?,
            school: self.school,
            name: self.name,
            validated: false,
            description: self.description,
            user_status: UserStatus::default().bits() as i64,
        })
    }
}

#[cfg(test)]
mod tests;
