use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    products::Products,
    schema::users,
};
use diesel::{prelude::*, sqlite::Sqlite};
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
        match user.get_info(conn)?.verify_passwd(passwd_provided) {
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

impl<'a> UserFinder<'a> {
    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<UserId>> {
        Self::new(conn, None).search()
    }

    pub fn list_info(conn: &'a SqliteConnection) -> Result<Vec<UserInfo>> {
        Self::new(conn, None).search_info()
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

    pub fn phone(mut self, phone_provided: &'a str) -> Self {
        use crate::schema::users::dsl::*;
        self.query = self.query.filter(school.eq(phone_provided));
        self
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, AsChangeset, Identifiable, Clone)]
#[table_name = "users"]
pub struct UserInfo {
    id: String,
    school: String,
    phone: String,
    hashed_passwd: String,
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

    /// Get a reference to the user info's phone.
    pub fn get_phone(&self) -> &str {
        &self.phone
    }

    pub fn verify_passwd(&self, passwd: impl AsRef<[u8]>) -> Result<bool> {
        Ok(bcrypt::verify(passwd, &self.hashed_passwd)?)
    }

    /// Set the user info's school.
    pub fn set_school(mut self, school: impl ToString) -> Self {
        self.school = school.to_string();
        self
    }

    /// Set the user info's phone.
    pub fn set_phone(mut self, phone: impl ToString) -> Self {
        self.phone = phone.to_string();
        self
    }

    pub fn set_password(mut self, raw_passwd: impl AsRef<[u8]>) -> Result<Self> {
        self.hashed_passwd = bcrypt::hash(raw_passwd, bcrypt::DEFAULT_COST)?;
        Ok(self)
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<UserInfo>(conn)?)
    }
}

// A struct used for update and insert
#[derive(Debug, Serialize, Deserialize, Insertable, AsChangeset, Identifiable, Clone)]
#[table_name = "users"]
pub struct UserInfoRef<'a> {
    id: &'a str,
    school: &'a str,
    phone: &'a str,
    // This is owned because we processed it
    hashed_passwd: String,
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
    pub school: String,
    pub phone: String,
    #[field(name = "password")]
    pub raw_passwd: String,
}

impl UserFormOwned {
    pub fn new<T: ToString>(id: T, school: T, phone: T, raw_passwd: T) -> Self {
        Self {
            id: id.to_string(),
            school: school.to_string(),
            phone: phone.to_string(),
            raw_passwd: raw_passwd.to_string(),
        }
    }

    pub fn to_ref(&self) -> Result<UserInfoRef> {
        let form = UserForm {
            id: &self.id,
            school: &self.school,
            phone: &self.phone,
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
    pub school: &'a str,
    pub phone: &'a str,
    #[field(name = "password")]
    pub raw_passwd: &'a str,
}

impl<'a> UserForm<'a> {
    pub fn new(id: &'a str, school: &'a str, phone: &'a str, raw_passwd: &'a str) -> Self {
        Self {
            id,
            school,
            phone,
            raw_passwd,
        }
    }

    pub fn to_ref(&self) -> Result<UserInfoRef<'a>> {
        let phone = phonenumber::parse(None, self.phone)?;
        match (
            phone.is_valid(),
            check_if_email_exists::syntax::check_syntax(self.id).is_valid_syntax,
        ) {
            (true, true) => Ok(UserInfoRef {
                id: self.id,
                hashed_passwd: bcrypt::hash(self.raw_passwd, bcrypt::DEFAULT_COST)?,
                school: self.school,
                phone: self.phone,
            }),
            _ => Err(SailsDbError::InvalidIdentity),
        }
    }
}

#[cfg(test)]
mod tests;
