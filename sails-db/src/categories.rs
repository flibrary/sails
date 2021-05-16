use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    schema::categories,
};
use diesel::{prelude::*, sqlite::Sqlite};
use rocket::FromForm;
use serde::{Deserialize, Serialize};

// A pseudo struct for managing the categories table.
pub struct Categories;

type BoxedQuery<'a> = categories::BoxedQuery<'a, Sqlite, categories::SqlType>;

impl Categories {
    pub fn list(conn: &SqliteConnection) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories.load::<Category>(conn)?)
    }

    pub fn list_top(conn: &SqliteConnection) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories
            .filter(parent_id.is_null())
            .load::<Category>(conn)?)
    }

    pub fn list_leaves(conn: &SqliteConnection) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories.filter(is_leaf.eq(true)).load::<Category>(conn)?)
    }

    fn by_id(id_provided: &'_ str) -> BoxedQuery<'_> {
        use crate::schema::categories::dsl::*;
        categories.into_boxed().filter(id.eq(id_provided))
    }

    pub fn find_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<Category> {
        use crate::schema::categories::dsl::*;
        Ok(categories
            .into_boxed()
            .filter(id.eq(id_provided))
            .get_result::<Category>(conn)?)
    }

    pub fn create(conn: &SqliteConnection, id_provided: impl ToString) -> Result<String> {
        use crate::schema::categories::dsl::*;
        let category = Category::new(id_provided);
        let id_cloned: String = category.id.clone();
        if let Ok(0) = Self::by_id(&category.id).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(categories)
                .values(category)
                .execute(conn)?
        } else {
            return Err(SailsDbError::CategoryExisted);
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

    pub fn delete_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<usize> {
        use crate::schema::categories::dsl::*;
        Ok(diesel::delete(categories.filter(id.eq(id_provided))).execute(conn)?)
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
    pub id: String,
    parent_id: Option<String>,
    is_leaf: bool,
}

impl Category {
    // Create a new leaf node with no parent_id
    pub fn new(id: impl ToString) -> Self {
        Self {
            id: id.to_string(),
            parent_id: None,
            is_leaf: true,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_leaf(&self) -> bool {
        self.is_leaf
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
