use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    schema::categories,
};
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

    pub fn delete_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<usize> {
        use crate::schema::categories::dsl::*;
        Ok(diesel::delete(categories.filter(id.eq(id_provided))).execute(conn)?)
    }

    pub fn delete_all(conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::categories::dsl::*;
        Ok(diesel::delete(categories).execute(conn)?)
    }

    pub fn create(conn: &SqliteConnection, name_provided: impl ToString) -> Result<String> {
        Self::create_with_id(conn, name_provided, Uuid::new_v4().to_string())
    }

    pub fn create_with_id(
        conn: &SqliteConnection,
        name_provided: impl ToString,
        id_provided: impl ToString,
    ) -> Result<String> {
        use crate::schema::categories::dsl::*;
        let category = Category::new(name_provided, id_provided);
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
        Self::update(conn, self_category)?;
        Self::update(conn, parent_category)?;
        Ok(())
    }

    pub fn update(conn: &SqliteConnection, category: Category) -> Result<()> {
        category.save_changes::<Category>(conn)?;
        Ok(())
    }
}

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "categories"]
pub struct Category {
    id: String,
    name: String,
    parent_id: Option<String>,
    is_leaf: bool,
}

impl Category {
    // Create a new leaf node with no parent_id
    pub fn new(name: impl ToString, id: impl ToString) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            parent_id: None,
            is_leaf: true,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
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

use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Id(Uuid),
    SubCategory(CategoryBuilderInner),
}

pub type CategoryBuilderInner = BTreeMap<String, Value>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CtgBuilder {
    #[serde(rename = "categories")]
    inner: CategoryBuilderInner,
}

impl CtgBuilder {
    pub fn build(self, conn: &SqliteConnection) -> Result<()> {
        fn walk(
            c: &diesel::SqliteConnection,
            parent_id: Option<&str>,
            current: &CategoryBuilderInner,
        ) -> Result<()> {
            for (name, value) in current {
                match value {
                    Value::Id(id) => {
                        println!("{:?}", id);
                        // Create the node
                        Categories::create_with_id(c, name, id)?;

                        // If there is a parent, link it back
                        if let Some(parent_id) = parent_id {
                            Categories::insert(c, &id.to_string(), parent_id)?;
                        } else {
                        }
                    }
                    Value::SubCategory(sub) => {
                        println!("{:?}", sub);
                        let self_id = Categories::create(c, name).unwrap();
                        if let Some(parent_id) = parent_id {
                            Categories::insert(c, &self_id.to_string(), parent_id)?;
                        } else {
                        }
                        walk(c, Some(&self_id), sub)?
                    }
                }
            }
            Ok(())
        }

        walk(conn, None, &self.inner)
    }
}

#[cfg(test)]
mod tests;
