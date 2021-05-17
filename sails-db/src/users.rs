use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    products::Products,
    schema::users,
};
use diesel::prelude::*;
use rocket::FromForm;
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
    // Return the ID of the user logged in
    pub fn login(
        conn: &SqliteConnection,
        id_provided: &str,
        passwd_provided: &str,
    ) -> Result<String> {
        use crate::schema::users::dsl::*;
        let user = users
            .filter(id.eq(id_provided))
            .first::<User>(conn)
            .optional()?;
        match user.clone().map(|u| u.verify_passwd(passwd_provided)) {
            Some(Ok(true)) => {
                // Successfully validated
                Ok(user.unwrap().id)
            }
            Some(Ok(false)) => {
                // User exists, but password is not right
                Err(SailsDbError::IncorrectPassword)
            }
            Some(Err(e)) => {
                // Some error occured during validation
                Err(e)
            }
            None => {
                // No user found
                Err(SailsDbError::UserNotFound)
            }
        }
    }

    pub fn register<T: ToString>(
        conn: &SqliteConnection,
        id_p: impl AsRef<str> + ToString,
        school_p: T,
        phone_p: impl AsRef<str> + ToString,
        passwd_p: T,
    ) -> Result<String> {
        use crate::schema::users::dsl::*;
        let user = User::new(id_p, school_p, phone_p, passwd_p)?;
        let id_cloned: String = user.id.clone();
        if let Ok(0) = users.filter(id.eq(&user.id)).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(users).values(user).execute(conn)?
        } else {
            return Err(SailsDbError::UserRegistered);
        };
        Ok(id_cloned)
    }

    // CRUD: DELETE
    pub fn delete_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<usize> {
        use crate::schema::users::dsl::*;
        // We need to also delete all the products associated with the user.
        Products::delete_by_seller(conn, id_provided)?;
        Ok(diesel::delete(users.filter(id.eq(id_provided))).execute(conn)?)
    }

    pub fn update(conn: &SqliteConnection, user: User) -> Result<()> {
        user.save_changes::<User>(conn)?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, FromForm)]
pub struct UpdateUser {
    school: Option<String>,
    phone: Option<String>,
    password: Option<String>,
}

impl Default for UpdateUser {
    fn default() -> Self {
        Self {
            school: None,
            phone: None,
            password: None,
        }
    }
}

/// A single user, corresponding to a row in the table `users`
#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
// We want to keep it intuitive
#[changeset_options(treat_none_as_null = "true")]
pub struct User {
    // This is actually email
    id: String,
    school: String,
    phone: String,
    hashed_passwd: String,
}

impl User {
    // Note that the passwd here is unhashed
    pub fn new<T: ToString>(
        id: impl AsRef<str> + ToString,
        school: T,
        phone: impl AsRef<str> + ToString,
        passwd: T,
    ) -> Result<Self> {
        let phone = phonenumber::parse(None, phone)?;
        match (
            phone.is_valid(),
            check_if_email_exists::syntax::check_syntax(id.as_ref()).is_valid_syntax,
        ) {
            (true, true) => Ok(Self {
                id: id.to_string(),
                hashed_passwd: bcrypt::hash(passwd.to_string(), bcrypt::DEFAULT_COST)?,
                school: school.to_string(),
                phone: phone.to_string(),
            }),
            _ => Err(SailsDbError::InvalidIdentity),
        }
    }

    pub fn update(&mut self, update: UpdateUser) -> Result<()> {
        if let Some(raw_passwd) = update.password {
            self.hashed_passwd = bcrypt::hash(raw_passwd, bcrypt::DEFAULT_COST)?;
        }
        if let Some(school) = update.school {
            self.school = school;
        }
        if let Some(phone) = update.phone {
            let phone = phonenumber::parse(None, phone)?;
            self.phone = if phone.is_valid() {
                phone.to_string()
            } else {
                return Err(SailsDbError::InvalidIdentity);
            };
        }
        Ok(())
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_phone(&self) -> &str {
        &self.phone
    }

    pub fn get_school(&self) -> &str {
        &self.school
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
