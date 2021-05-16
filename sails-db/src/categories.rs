use crate::{error::SailsDbResult as Result, schema::categories};
use diesel::{prelude::*, sqlite::Sqlite};
use rocket::FromForm;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// A pseudo struct for managing the categories table.
pub struct Categories;

type BoxedQuery<'a> = categories::BoxedQuery<'a, Sqlite, categories::SqlType>;

impl Categories {
    pub fn list(conn: &SqliteConnection) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories.load::<Category>(conn)?)
    }

    fn by_id(id_provided: &'_ str) -> BoxedQuery<'_> {
        use crate::schema::categories::dsl::*;
        categories.into_boxed().filter(id.eq(id_provided))
    }

    pub fn create(conn: &SqliteConnection, ctgname_provided: impl ToString) -> Result<String> {
        use crate::schema::categories::dsl::*;
        let category = Category::new(ctgname_provided);
        let id_cloned: String = category.id.clone();
        if let Ok(0) = Self::by_id(&category.id).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(categories)
                .values(category)
                .execute(conn)?
        } else {
            // This can never happen because we are using UUID.
            unreachable!()
        };
        Ok(id_cloned)
    }

    pub fn subcategory(conn: &SqliteConnection, id_provided: &str) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories
            .filter(parent_id.eq(id_provided))
            .load::<Category>(conn)?)
    }

    pub fn insert(conn: &SqliteConnection, self_id: &str, parent_id_provided: &str) -> Result<()> {
        let mut self_category = Self::by_id(self_id).first::<Category>(conn)?;
        let mut parent_category = Self::by_id(parent_id_provided).first::<Category>(conn)?;
        self_category.insert(&mut parent_category);
        Self::create_or_update(conn, self_category)?;
        Self::create_or_update(conn, parent_category)?;
        Ok(())
    }

    pub fn create_or_update(conn: &SqliteConnection, category: Category) -> Result<()> {
        use crate::schema::categories::dsl::*;

        if let Ok(0) = categories
            .filter(id.eq(&category.id))
            .count()
            .get_result(conn)
        {
            // This means that we have to insert
            diesel::insert_into(categories)
                .values(category)
                .execute(conn)?;
        } else {
            category.save_changes::<Category>(conn)?;
        };
        Ok(())
    }
}

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "categories"]
pub struct Category {
    id: String,
    pub ctgname: String,
    parent_id: Option<String>,
    is_leaf: bool,
}

impl Category {
    // Create a new leaf node with no parent_id
    pub fn new(ctgname: impl ToString) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            ctgname: ctgname.to_string(),
            parent_id: None,
            is_leaf: true,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    // To insert a node between A and B, first insert the node to A, then insert B to the node.
    pub(crate) fn insert(&mut self, parent: &mut Category) {
        self.parent_id = Some(parent.id.clone());
        // If previously the parent was a leaf node, we then have to change it.
        if parent.is_leaf {
            parent.is_leaf = false;
        }
    }
}

#[cfg(test)]
mod tests;
