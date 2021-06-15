// We have to ensure: all the places where category can be supplied has to be using type Category instead of String.
// If we cannot ensure on derivations like those did by serde, we then have to use isolation types to ensure it on a type level

use crate::{
    categories::{Categories, CtgTrait, LeafCategory},
    error::{SailsDbError, SailsDbResult as Result},
    schema::products,
    users::UserId,
    Cmp, Order,
};
use diesel::{prelude::*, sqlite::Sqlite};
use rocket::FromForm;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Identifiable, Queryable, Clone)]
#[table_name = "products"]
pub struct ProductId {
    id: String,
}

impl ProductId {
    pub fn to_uuid(&self) -> Result<Uuid> {
        Ok(<Uuid as std::str::FromStr>::from_str(&self.id)?)
    }

    pub fn get_info(&self, conn: &SqliteConnection) -> Result<ProductInfo> {
        use crate::schema::products::dsl::*;
        Ok(products
            .filter(id.eq(&self.id))
            .first::<ProductInfo>(conn)?)
    }

    pub fn delete(self, conn: &SqliteConnection) -> Result<()> {
        use crate::schema::products::dsl::*;
        diesel::delete(products.filter(id.eq(&self.id))).execute(conn)?;
        Ok(())
    }

    pub fn update(&self, conn: &SqliteConnection, info: SafeIncompleteProduct) -> Result<()> {
        diesel::update(self).set(info).execute(conn)?;
        Ok(())
    }

    pub fn update_owned(
        &self,
        conn: &SqliteConnection,
        info: SafeIncompleteProductOwned,
    ) -> Result<()> {
        diesel::update(self).set(info).execute(conn)?;
        Ok(())
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }
}

/// A pseudo products used to manage table `products`
pub struct Products;

impl Products {
    pub fn delete_by_seller(conn: &SqliteConnection, seller: &UserId) -> Result<usize> {
        use crate::schema::products::dsl::*;
        Ok(diesel::delete(products.filter(seller_id.eq(seller.get_id()))).execute(conn)?)
    }
}

type BoxedQuery<'a> = products::BoxedQuery<'a, Sqlite, products::SqlType>;

/// A search query helper (builder)
pub struct ProductFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

impl<'a> ProductFinder<'a> {
    pub fn list_info(conn: &'a SqliteConnection) -> Result<Vec<ProductInfo>> {
        Self::new(conn, None).search_info()
    }

    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<ProductId>> {
        Self::new(conn, None).search()
    }

    pub fn new(conn: &'a SqliteConnection, query: Option<BoxedQuery<'a>>) -> Self {
        use crate::schema::products::dsl::*;
        if let Some(q) = query {
            Self { conn, query: q }
        } else {
            Self {
                conn,
                query: products.into_boxed(),
            }
        }
    }

    pub fn search(self) -> Result<Vec<ProductId>> {
        use crate::schema::products::dsl::*;
        Ok(self
            .query
            .select(id)
            .load::<String>(self.conn)?
            .into_iter()
            .map(|x| ProductId { id: x })
            .collect())
    }

    pub fn search_info(self) -> Result<Vec<ProductInfo>> {
        Ok(self.query.load::<ProductInfo>(self.conn)?)
    }

    pub fn first(self) -> Result<ProductId> {
        use crate::schema::products::dsl::*;
        Ok(ProductId {
            id: self.query.select(id).first::<String>(self.conn)?,
        })
    }

    pub fn first_info(self) -> Result<ProductInfo> {
        Ok(self.query.first::<ProductInfo>(self.conn)?)
    }

    pub fn id(mut self, id_provided: &'a str) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(id.eq(id_provided));
        self
    }

    pub fn prodname(mut self, prodname_provided: &'a str) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(prodname.eq(prodname_provided));
        self
    }

    pub fn seller(mut self, seller: &'a str) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(seller_id.eq(seller));
        self
    }

    pub fn category(mut self, category_provided: &'a LeafCategory) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(category.eq(category_provided.id()));
        self
    }

    pub fn price(mut self, price_provided: i64, cmp: Cmp) -> Self {
        use crate::schema::products::dsl::*;
        match cmp {
            Cmp::GreaterThan => self.query = self.query.filter(price.gt(price_provided)),
            Cmp::LessThan => self.query = self.query.filter(price.lt(price_provided)),
            Cmp::GreaterEqual => self.query = self.query.filter(price.ge(price_provided)),
            Cmp::LessEqual => self.query = self.query.filter(price.le(price_provided)),
        }
        self
    }

    pub fn order_by_price(mut self, order: Order) -> Self {
        use crate::schema::products::dsl::*;
        match order {
            Order::Asc => self.query = self.query.order(price.asc()),
            Order::Desc => self.query = self.query.order(price.desc()),
        }
        self
    }
}

pub trait ToSafe<T> {
    fn verify(self, conn: &SqliteConnection) -> Result<T>;
}

// category-verified product
#[derive(Debug, Clone, AsChangeset)]
#[table_name = "products"]
pub struct SafeIncompleteProductOwned {
    // This is the ID (UUID) of the category
    pub category: String,
    pub prodname: String,
    pub price: i64,
    pub description: String,
}

// TODO: We can ensure that category does exist, but we cannot ensure that category is the leaf
#[derive(Debug, Serialize, Deserialize, Clone, FromForm)]
pub struct IncompleteProductOwned {
    // This is the ID (UUID) of the category
    pub category: String,
    pub prodname: String,
    pub price: i64,
    pub description: String,
}

impl ToSafe<SafeIncompleteProductOwned> for IncompleteProductOwned {
    fn verify(self, conn: &SqliteConnection) -> Result<SafeIncompleteProductOwned> {
        let ctg = Categories::find_by_id(conn, &self.category)?;
        if ctg.is_leaf() {
            Ok(SafeIncompleteProductOwned {
                category: self.category,
                prodname: self.prodname,
                price: self.price,
                description: self.description,
            })
        } else {
            Err(SailsDbError::NonLeafCategory)
        }
    }
}

impl IncompleteProductOwned {
    pub fn new<T: ToString>(
        category: &LeafCategory,
        prodname: T,
        price: i64,
        description: T,
    ) -> Self {
        Self {
            category: category.id().to_string(),
            prodname: prodname.to_string(),
            price,
            description: description.to_string(),
        }
    }

    pub fn create(&self, conn: &SqliteConnection, seller: &UserId) -> Result<ProductId> {
        let refed = IncompleteProduct {
            category: &self.category,
            prodname: &self.prodname,
            price: self.price,
            description: &self.description,
        };
        refed.create(conn, seller)
    }
}

// category-verified product
#[derive(Debug, Clone, AsChangeset)]
#[table_name = "products"]
pub struct SafeIncompleteProduct<'a> {
    pub category: &'a str,
    pub prodname: &'a str,
    pub price: i64,
    pub description: &'a str,
}

impl<'a> ToSafe<SafeIncompleteProduct<'a>> for IncompleteProduct<'a> {
    fn verify(self, conn: &SqliteConnection) -> Result<SafeIncompleteProduct<'a>> {
        let ctg = Categories::find_by_id(conn, &self.category)?;
        if ctg.is_leaf() {
            Ok(SafeIncompleteProduct {
                category: self.category,
                prodname: self.prodname,
                price: self.price,
                description: self.description,
            })
        } else {
            Err(SailsDbError::NonLeafCategory)
        }
    }
}

impl<'a> SafeIncompleteProduct<'a> {
    pub fn create(self, conn: &SqliteConnection, seller: &UserId) -> Result<ProductId> {
        use crate::schema::products::dsl::*;
        let id_cloned = Uuid::new_v4().to_string();
        let value = (
            id.eq(&id_cloned),
            seller_id.eq(seller.get_id()),
            category.eq(self.category),
            prodname.eq(self.prodname),
            price.eq(self.price),
            description.eq(self.description),
        );
        diesel::insert_into(products).values(value).execute(conn)?;
        Ok(ProductId { id: id_cloned })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, FromForm)]
pub struct IncompleteProduct<'a> {
    pub category: &'a str,
    pub prodname: &'a str,
    pub price: i64,
    pub description: &'a str,
}

impl<'a> IncompleteProduct<'a> {
    pub fn new(
        category: &'a LeafCategory,
        prodname: &'a str,
        price: i64,
        description: &'a str,
    ) -> Self {
        Self {
            category: category.id(),
            prodname,
            price,
            description,
        }
    }

    pub fn create(self, conn: &SqliteConnection, seller: &UserId) -> Result<ProductId> {
        self.verify(conn)?.create(conn, seller)
    }
}

/// A single product info entry, corresponding to a row in the table `products`
#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone,
)]
#[table_name = "products"]
pub struct ProductInfo {
    id: String,
    seller_id: String,
    category: String,
    prodname: String,
    price: i64,
    description: String,
}

impl ProductInfo {
    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<ProductInfo>(conn)?)
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_seller_id(&self) -> &str {
        &self.seller_id
    }

    pub fn get_description(&self) -> &str {
        &self.description
    }

    pub fn get_category_id(&self) -> &str {
        &self.category
    }

    pub fn get_prodname(&self) -> &str {
        &self.prodname
    }

    pub fn get_price(&self) -> i64 {
        self.price
    }

    /// Set the product info's seller id.
    pub fn set_seller_id(mut self, seller_id: impl ToString) -> Self {
        self.seller_id = seller_id.to_string();
        self
    }

    /// Set the product info's category.
    pub fn set_category(mut self, category: &LeafCategory) -> Result<Self> {
        self.category = category.id().to_string();
        Ok(self)
    }

    /// Set the product info's prodname.
    pub fn set_prodname(mut self, prodname: impl ToString) -> Self {
        self.prodname = prodname.to_string();
        self
    }

    /// Set the product info's price.
    pub fn set_price(mut self, price: i64) -> Self {
        self.price = price;
        self
    }

    /// Set the product info's description.
    pub fn set_description(mut self, description: impl ToString) -> Self {
        self.description = description.to_string();
        self
    }
}

#[cfg(test)]
mod tests;
