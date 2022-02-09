use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    schema::categories,
};
use delegate_attr::delegate;
use diesel::prelude::*;
use rocket::FromForm;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, num::NonZeroU32, sync::Arc};
use uuid::Uuid;

// A pseudo struct for managing the categories table.
pub struct Categories;

impl Categories {
    pub fn list_all(conn: &SqliteConnection) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories.load::<Category>(conn)?)
    }

    pub fn list_top(conn: &SqliteConnection) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories
            .filter(parent_id.is_null())
            .load::<Category>(conn)?)
    }

    pub fn list_leaves<T: CtgTrait>(
        conn: &SqliteConnection,
        root: Option<&T>,
    ) -> Result<Vec<LeafCategory>> {
        use crate::schema::categories::dsl::*;
        if let Some(root) = root {
            if root.is_leaf() {
                // This means we are at the bottom of the search
                // ... and we shall return the leaf
                Ok(vec![root.clone().into_leaf()?])
            } else {
                let mut v = Vec::new();
                for child in categories
                    .filter(parent_id.eq(CtgTrait::id(root)))
                    .load::<Category>(conn)?
                {
                    v = [v, Categories::list_leaves(conn, Some(&child))?].concat();
                }
                Ok(v)
            }
        } else {
            Ok(categories
                .filter(is_leaf.eq(true))
                .load::<Category>(conn)?
                .into_iter()
                .map(|x| x.into_leaf().unwrap())
                .collect())
        }
    }

    pub fn find_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<Category> {
        use crate::schema::categories::dsl::*;
        Ok(categories
            .into_boxed()
            .filter(id.eq(id_provided))
            .first::<Category>(conn)?)
    }

    /// Note: this returns the first category matching the name.
    /// Name is NOT guaranteed to be unique. Whenever possible, use find_by_id instead.
    pub fn find_by_name(conn: &SqliteConnection, name_provided: &str) -> Result<Category> {
        use crate::schema::categories::dsl::*;
        Ok(categories
            .into_boxed()
            .filter(name.eq(name_provided))
            .first::<Category>(conn)?)
    }

    pub fn delete_all(conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::categories::dsl::*;
        Ok(diesel::delete(categories).execute(conn)?)
    }
}

// A trait governing both LeafCategory and Category
pub trait CtgTrait: Clone + Sized {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn parent_id(&self) -> Option<&str>;
    fn is_leaf(&self) -> bool;
    fn into_leaf(self) -> Result<LeafCategory>;
    // Leaf category should only be allowed to insert to category, otherwise type leakage may occur
    fn insert(&mut self, conn: &SqliteConnection, parent: &mut Category) -> Result<()>;
    fn update(&self, conn: &SqliteConnection) -> Result<Self>;
    fn delete(self, conn: &SqliteConnection) -> Result<usize>;
}

// A type-level wraper to ensuer that the category is leaf
#[derive(Clone)]
pub struct LeafCategory(Category);

impl LeafCategory {
    pub fn into_category(self) -> Category {
        self.0
    }

    // Only price at leaf category is meaningful
    pub fn get_price(&self) -> u32 {
        self.0.price as u32
    }
}

// Rustfmt tends to remove pub
impl CtgTrait for LeafCategory {
    fn id(&self) -> &str {
        CtgTrait::id(&self.0)
    }
    #[delegate(self.0)]
    fn name(&self) -> &str;
    #[delegate(self.0)]
    fn parent_id(&self) -> Option<&str>;
    #[delegate(self.0)]
    fn insert(&mut self, conn: &SqliteConnection, parent: &mut Category) -> Result<()>;
    fn is_leaf(&self) -> bool {
        true
    }
    fn update(&self, conn: &SqliteConnection) -> Result<Self> {
        Ok(LeafCategory(self.0.update(conn)?))
    }

    #[delegate(self.0)]
    fn delete(self, conn: &SqliteConnection) -> Result<usize>;

    fn into_leaf(self) -> Result<LeafCategory> {
        Ok(self)
    }
}

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "categories"]
pub struct Category {
    id: String,
    name: String,
    price: i64,
    parent_id: Option<String>,
    is_leaf: bool,
}

impl Category {
    fn new(name: impl ToString, id: impl ToString, price: NonZeroU32) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            price: price.get() as i64,
            parent_id: None,
            is_leaf: true,
        }
    }

    // Create a new category with a random UUID
    pub fn create(
        conn: &SqliteConnection,
        name_provided: impl ToString,
        price_provided: u32,
    ) -> Result<Self> {
        Self::create_with_id(
            conn,
            name_provided,
            price_provided,
            Uuid::new_v4().to_string(),
        )
    }

    // Create a new category with a specific UUID
    pub fn create_with_id(
        conn: &SqliteConnection,
        name_provided: impl ToString,
        price_provided: u32,
        id_provided: impl ToString,
    ) -> Result<Self> {
        let price_provided =
            NonZeroU32::new(price_provided).ok_or(SailsDbError::IllegalPriceOrQuantity)?;

        use crate::schema::categories::dsl::*;
        let category = Category::new(name_provided, id_provided, price_provided);

        if let Ok(0) = categories
            .filter(id.eq(&category.id))
            .count()
            .get_result(conn)
        {
            // This means that we have to insert
            diesel::insert_into(categories)
                .values(&category)
                .execute(conn)?
        } else {
            return Err(SailsDbError::CategoryExisted);
        };
        Ok(category)
    }

    pub fn set_leaf(&mut self, is_leaf: bool) {
        self.is_leaf = is_leaf;
    }

    pub fn subcategory(&self, conn: &SqliteConnection) -> Result<Vec<Category>> {
        use crate::schema::categories::dsl::*;
        Ok(categories
            .filter(parent_id.eq(&self.id))
            .load::<Category>(conn)?)
    }
}

impl CtgTrait for Category {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn is_leaf(&self) -> bool {
        self.is_leaf
    }

    // To insert a node between A and B, first insert the node to A, then insert B to the node.
    fn insert(&mut self, conn: &SqliteConnection, parent: &mut Category) -> Result<()> {
        self.parent_id = Some(CtgTrait::id(parent).to_string());
        if parent.is_leaf() {
            parent.set_leaf(false);
        }
        self.update(conn)?;
        parent.update(conn)?;
        Ok(())
    }

    fn parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref()
    }

    fn update(&self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<Category>(conn)?)
    }

    fn delete(self, conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::categories::dsl::*;
        Ok(diesel::delete(categories.filter(id.eq(self.id))).execute(conn)?)
    }

    fn into_leaf(self) -> Result<LeafCategory> {
        if self.is_leaf() {
            Ok(LeafCategory(self))
        } else {
            Err(SailsDbError::NonLeafCategory)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Id { id: Uuid, price: u32 },
    SubCategory(CategoryBuilderInner),
}

pub type CategoryBuilderInner = HashMap<Arc<str>, Value>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CtgBuilder {
    #[serde(rename = "categories")]
    inner: CategoryBuilderInner,
}

impl CtgBuilder {
    pub fn new(inner: CategoryBuilderInner) -> Self {
        Self { inner }
    }

    pub fn inner(self) -> CategoryBuilderInner {
        self.inner
    }

    pub fn build(self, conn: &SqliteConnection) -> Result<()> {
        fn walk(
            c: &diesel::SqliteConnection,
            parent: Option<Category>,
            current: &CategoryBuilderInner,
        ) -> Result<()> {
            for (name, value) in current {
                match value {
                    Value::Id { id, price } => {
                        // Create the node
                        let mut self_ctg = Category::create_with_id(c, name, *price, id)?;

                        // If there is a parent, link it back
                        if let Some(mut parent) = parent.clone() {
                            self_ctg.insert(c, &mut parent)?;
                        }
                    }
                    Value::SubCategory(sub) => {
                        // Price here is meaningless as it is not the leaf category
                        // And it cannot be retrieved as well
                        let mut self_ctg = Category::create(c, name, 1).unwrap();
                        if let Some(mut parent) = parent.clone() {
                            self_ctg.insert(c, &mut parent)?;
                        }
                        walk(c, Some(self_ctg), sub)?
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
