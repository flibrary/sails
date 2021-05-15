use super::users::User;
use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    schema::products,
    Cmp, Order,
};
use diesel::{prelude::*, sqlite::Sqlite};
use serde::{Deserialize, Serialize};
use std::num::NonZeroI64;
use uuid::Uuid;

/// A pseudo products used to manage table `products`
pub struct Products;

impl Products {
    // CRUD: READ. For convenience
    pub fn list(conn: &SqliteConnection) -> Result<Vec<Product>> {
        ProductFinder::new(conn, None).search()
    }

    // A convenient helper to create the product
    pub fn create_product<T: ToString>(
        conn: &SqliteConnection,
        seller_p: &User,
        prodname_p: T,
        price_p: NonZeroI64,
        description_p: T,
    ) -> Result<String> {
        use crate::schema::products::dsl::*;
        let product = Product::new(seller_p, prodname_p, price_p, description_p);
        let id_cloned: String = product.id().to_string();
        if let Ok(0) = products.filter(id.eq(&product.id)).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(products)
                .values(product)
                .execute(conn)?
        } else {
            return Err(SailsDbError::UserRegistered);
        };
        Ok(id_cloned)
    }

    // CRUD: DELETE
    pub fn delete_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<usize> {
        use crate::schema::products::dsl::*;
        Ok(diesel::delete(products.filter(id.eq(id_provided))).execute(conn)?)
    }

    // CRUD: UPDATE AND CREATE
    pub fn create_or_update(conn: &SqliteConnection, product: Product) -> Result<()> {
        use crate::schema::products::dsl::*;

        if let Ok(0) = products.filter(id.eq(&product.id)).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(products)
                .values(product)
                .execute(conn)?
        } else {
            diesel::update(products).set(product).execute(conn)?
        };
        Ok(())
    }
}

type BoxedQuery<'a> = products::BoxedQuery<'a, Sqlite, products::SqlType>;

/// A search query helper (builder)
pub struct ProductFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

impl<'a> ProductFinder<'a> {
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

    pub fn search(self) -> Result<Vec<Product>> {
        Ok(self.query.load::<Product>(self.conn)?)
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

    pub fn seller(mut self, seller: &'a User) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(seller_id.eq(seller.id()));
        self
    }

    pub fn price(mut self, price_provided: NonZeroI64, cmp: Cmp) -> Self {
        use crate::schema::products::dsl::*;
        match cmp {
            Cmp::GreaterThan => self.query = self.query.filter(price.gt(price_provided.get())),
            Cmp::LessThan => self.query = self.query.filter(price.lt(price_provided.get())),
            Cmp::GreaterEqual => self.query = self.query.filter(price.ge(price_provided.get())),
            Cmp::LessEqual => self.query = self.query.filter(price.le(price_provided.get())),
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

/// A single user, corresponding to a row in the table `products`
#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone,
)]
// We want to keep it intuitive
#[changeset_options(treat_none_as_null = "true")]
pub struct Product {
    id: String,
    seller_id: String,
    pub prodname: String,
    // Price should not be negative
    price: i64,
    pub description: String,
}

impl Product {
    // This prevent on a type level that seller_id and price are valid
    pub fn new<T: ToString>(seller: &User, prodname: T, price: NonZeroI64, description: T) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            seller_id: seller.id.clone(),
            prodname: prodname.to_string(),
            price: price.get(),
            description: description.to_string(),
        }
    }

    pub fn seller_id(&self) -> &str {
        &self.seller_id
    }

    pub fn set_seller_id(&mut self, seller: &User) {
        self.seller_id = seller.id.clone();
    }

    pub fn price(&self) -> u32 {
        self.price as u32
    }

    pub fn set_price(&mut self, price: NonZeroI64) {
        self.price = price.get();
    }
}

#[cfg(test)]
mod tests;
