use crate::{error::SailsDbResult as Result, schema::products, Cmp, Order};
use diesel::{prelude::*, sqlite::Sqlite};
use rocket::FromForm;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A pseudo products used to manage table `products`
pub struct Products;

impl Products {
    // CRUD: READ. For convenience
    pub fn list(conn: &SqliteConnection) -> Result<Vec<Product>> {
        ProductFinder::new(conn, None).search()
    }

    // create the product
    // This ensures that category cannot be optional
    pub fn create<T: ToString>(
        conn: &SqliteConnection,
        seller_p: T,
        category_p: T,
        prodname_p: T,
        price_p: i64,
        description_p: T,
    ) -> Result<String> {
        use crate::schema::products::dsl::*;
        let product = Product::new(seller_p, category_p, prodname_p, price_p, description_p);
        let id_cloned: String = product.id.clone();
        if let Ok(0) = products.filter(id.eq(&product.id)).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(products)
                .values(product)
                .execute(conn)?
        } else {
            // This can never happen because we are using UUID.
            unreachable!()
        };
        Ok(id_cloned)
    }

    // CRUD: DELETE
    // We somehow cannot get product finder to help us to delete.
    pub fn delete_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<usize> {
        use crate::schema::products::dsl::*;
        Ok(diesel::delete(products.filter(id.eq(id_provided))).execute(conn)?)
    }

    pub fn delete_by_seller(conn: &SqliteConnection, seller: &str) -> Result<usize> {
        use crate::schema::products::dsl::*;
        Ok(diesel::delete(products.filter(seller_id.eq(seller))).execute(conn)?)
    }

    // CRUD: UPDATE
    pub fn update(conn: &SqliteConnection, product: Product) -> Result<()> {
        product.save_changes::<Product>(conn)?;
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

    pub fn seller(mut self, seller: &'a str) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(seller_id.eq(seller));
        self
    }

    pub fn category(mut self, category_provided: &'a str) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(category.eq(category_provided));
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

#[derive(Debug, Serialize, Deserialize, Clone, FromForm)]
pub struct UpdateProduct {
    pub category: Option<String>,
    pub prodname: Option<String>,
    pub price: Option<i64>,
    pub description: Option<String>,
}

impl Default for UpdateProduct {
    fn default() -> Self {
        Self {
            category: None,
            prodname: None,
            price: None,
            description: None,
        }
    }
}

/// A single user, corresponding to a row in the table `products`
#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
// We want to keep it intuitive
#[changeset_options(treat_none_as_null = "true")]
pub struct Product {
    id: String,
    seller_id: String,
    category: String,
    prodname: String,
    price: i64,
    description: String,
}

impl Product {
    pub fn new<T: ToString>(
        seller_id: T,
        category: T,
        prodname: T,
        price: i64,
        description: T,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            seller_id: seller_id.to_string(),
            category: category.to_string(),
            prodname: prodname.to_string(),
            price,
            description: description.to_string(),
        }
    }

    pub fn update(&mut self, update: UpdateProduct) {
        if let Some(category) = update.category {
            self.category = category
        }
        if let Some(prodname) = update.prodname {
            self.prodname = prodname
        }
        if let Some(price) = update.price {
            self.price = price
        }
        if let Some(desc) = update.description {
            self.description = desc
        }
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

    pub fn get_category(&self) -> &str {
        &self.category
    }

    pub fn get_prodname(&self) -> &str {
        &self.prodname
    }

    pub fn get_price(&self) -> i64 {
        self.price
    }
}

#[cfg(test)]
mod tests;
