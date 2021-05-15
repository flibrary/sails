use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    schema::users,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

/// A pseudo struct used to manage the table `users`
pub struct Users;

impl Users {
    // CRUD: READ
    pub fn list(conn: &SqliteConnection) -> Result<Vec<User>> {
        use crate::schema::users::dsl::*;
        Ok(users.load::<User>(conn)?)
    }

    // CRUD: READ
    pub fn find_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<User> {
        use crate::schema::users::dsl::*;
        Ok(users.filter(id.eq(id_provided)).first::<User>(conn)?)
    }

    // An convenient method to login the user
    pub fn login(
        conn: &SqliteConnection,
        id_provided: &str,
        passwd_provided: &str,
    ) -> Result<Option<User>> {
        use crate::schema::users::dsl::*;
        let user = users.filter(id.eq(id_provided)).first::<User>(conn)?;
        if user.verify_passwd(passwd_provided)? {
            Ok(Some(user))
        } else {
            Ok(None)
        }
    }

    pub fn register<T: ToString>(
        conn: &SqliteConnection,
        id_p: T,
        email_p: Option<T>,
        school_p: T,
        phone_p: impl AsRef<str>,
        passwd_p: T,
    ) -> Result<()> {
        use crate::schema::users::dsl::*;
        let user = User::new(id_p, email_p, school_p, phone_p.as_ref(), passwd_p)?;
        if let Ok(0) = users.filter(id.eq(&user.id)).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(users).values(user).execute(conn)?
        } else {
            return Err(SailsDbError::UserRegistered);
        };
        Ok(())
    }

    // CRUD: DELETE
    pub fn delete_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<usize> {
        use crate::schema::users::dsl::*;
        Ok(diesel::delete(users.filter(id.eq(id_provided))).execute(conn)?)
    }

    // CRUD: UPDATE AND CREATE
    pub fn create_or_update(conn: &SqliteConnection, user: User) -> Result<()> {
        use crate::schema::users::dsl::*;

        if let Ok(0) = users.filter(id.eq(&user.id)).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(users).values(user).execute(conn)?
        } else {
            diesel::update(users).set(user).execute(conn)?
        };
        Ok(())
    }
}

/// A single user, corresponding to a row in the table `users`
#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone,
)]
// We want to keep it intuitive
#[changeset_options(treat_none_as_null = "true")]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub school: String,
    pub phone: String,
    hashed_passwd: String,
}

impl User {
    // Note that the passwd here is unhashed
    pub fn new<T: ToString>(
        id: T,
        email: Option<T>,
        school: T,
        phone: &str,
        passwd: T,
    ) -> Result<Self> {
        let phone = phonenumber::parse(None, phone)?;
        if phone.is_valid() {
            Ok(Self {
                id: id.to_string(),
                hashed_passwd: bcrypt::hash(passwd.to_string(), bcrypt::DEFAULT_COST)?,
                email: email.map(|s| s.to_string()),
                school: school.to_string(),
                phone: phone.to_string(),
            })
        } else {
            Err(SailsDbError::InvalidPhoneNumber)
        }
    }

    pub fn verify_passwd(&self, passwd: impl AsRef<[u8]>) -> Result<bool> {
        Ok(bcrypt::verify(passwd, &self.hashed_passwd)?)
    }

    pub fn change_passwd(&mut self, passwd: impl AsRef<[u8]>) -> Result<()> {
        self.hashed_passwd = bcrypt::hash(passwd, bcrypt::DEFAULT_COST)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
