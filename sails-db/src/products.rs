// We have to ensure: all the places where category can be supplied has to be using type Category instead of String.
// If we cannot ensure on derivations like those did by serde, we then have to use isolation types to ensure it on a type level

use std::num::NonZeroU32;

use crate::{
    categories::{Categories, CtgTrait, LeafCategory},
    enums::{ProductStatus, UserStatus},
    error::{SailsDbError, SailsDbResult as Result},
    schema::products,
    tags::TagMappingFinder,
    users::UserId,
    Cmp, Order,
};
use diesel::{prelude::*, sql_types::Bool, sqlite::Sqlite};
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

    // Deletion would not be allowed if the product is referenced in the transaction due to the foreign key constraints
    pub fn delete(self, conn: &SqliteConnection) -> Result<()> {
        use crate::schema::products::dsl::*;
        // Delete the tags mapping associated with the product
        TagMappingFinder::new(conn, None).delete_by_product(&self)?;
        diesel::delete(products.filter(id.eq(&self.id))).execute(conn)?;
        Ok(())
    }

    // IncompleteProduct update should only be allowed if the book is not sold (frozen)
    pub fn update(&self, conn: &SqliteConnection, info: SafeIncompleteProduct) -> Result<()> {
        diesel::update(self).set(info).execute(conn)?;
        Ok(())
    }

    // IncompleteProduct update should only be allowed if the book is not sold (frozen)
    // This is safe to update because creation and update has been seperated and it will not fallback to default.
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
    pub fn delete_by_seller(conn: &SqliteConnection, seller: &UserId) -> Result<()> {
        for p in ProductFinder::new(conn, None).seller(seller).search()? {
            p.delete(conn)?;
        }
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

    // User owns the product
    pub fn seller(mut self, seller: &'a UserId) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self.query.filter(seller_id.eq(seller.get_id()));
        self
    }

    pub fn category(mut self, category_provided: &'a impl CtgTrait) -> Result<Self> {
        use crate::schema::products::{dsl::*, table};
        let mut criteria: Box<dyn BoxableExpression<table, Sqlite, SqlType = Bool>> =
            Box::new(false.into_sql::<Bool>());
        for ctg in Categories::list_leaves(self.conn, Some(category_provided))? {
            criteria = Box::new(criteria.or(category.eq(CtgTrait::id(&ctg).to_string())));
        }
        self.query = self.query.filter(criteria);
        Ok(self)
    }

    pub fn price(mut self, price_provided: u32, cmp: Cmp) -> Self {
        let price_provided = price_provided as i64;
        use crate::schema::products::dsl::*;
        match cmp {
            Cmp::GreaterThan => self.query = self.query.filter(price.gt(price_provided)),
            Cmp::LessThan => self.query = self.query.filter(price.lt(price_provided)),
            Cmp::GreaterEqual => self.query = self.query.filter(price.ge(price_provided)),
            Cmp::LessEqual => self.query = self.query.filter(price.le(price_provided)),
            Cmp::NotEqual => self.query = self.query.filter(price.ne(price_provided)),
            Cmp::Equal => self.query = self.query.filter(price.eq(price_provided)),
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

    pub fn status(mut self, status: ProductStatus, cmp: Cmp) -> Self {
        use crate::schema::products::dsl::*;
        match cmp {
            Cmp::Equal => self.query = self.query.filter(product_status.eq(status)),
            Cmp::NotEqual => self.query = self.query.filter(product_status.ne(status)),
            // Currently it makes no sense for us to do so
            _ => unimplemented!(),
        }
        self
    }

    pub fn allowed(mut self) -> Self {
        use crate::schema::products::dsl::*;
        self.query = self
            .query
            .filter(product_status.ne(ProductStatus::Disabled));
        self
    }
}

pub trait ToSafe<T> {
    fn verify(self, conn: &SqliteConnection) -> Result<T>;
}

// category-verified product
// Since this product is acting as a changeset to the database, we have to use i64 here.
#[derive(Debug, Clone, AsChangeset)]
#[table_name = "products"]
pub struct SafeIncompleteProductOwned {
    // This is the ID (UUID) of the category
    pub category: String,
    pub prodname: String,
    pub price: i64,
    pub quantity: i64,
    pub description: String,
}

// TODO: We can ensure that category does exist, but we cannot ensure that category is the leaf
#[derive(Debug, Serialize, Deserialize, Clone, FromForm)]
pub struct IncompleteProductOwned {
    // This is the ID (UUID) of the category
    pub category: String,
    pub prodname: String,
    pub price: u32,
    pub quantity: NonZeroU32,
    pub description: String,
}

impl ToSafe<SafeIncompleteProductOwned> for IncompleteProductOwned {
    fn verify(self, conn: &SqliteConnection) -> Result<SafeIncompleteProductOwned> {
        let ctg = Categories::find_by_id(conn, &self.category)?;
        if ctg.is_leaf() {
            Ok(SafeIncompleteProductOwned {
                category: self.category,
                prodname: self.prodname,
                price: self.price as i64,
                quantity: self.quantity.get() as i64,
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
        price: u32,
        quantity: u32,
        description: T,
    ) -> Result<Self> {
        let quantity = NonZeroU32::new(quantity).ok_or(SailsDbError::IllegalPriceOrQuantity)?;
        Ok(Self {
            category: category.id().to_string(),
            prodname: prodname.to_string(),
            price,
            quantity,
            description: description.to_string(),
        })
    }

    pub fn create(&self, conn: &SqliteConnection, seller: &UserId) -> Result<ProductId> {
        let refed = IncompleteProduct {
            category: &self.category,
            prodname: &self.prodname,
            price: self.price,
            quantity: self.quantity,
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
    pub quantity: i64,
    pub description: &'a str,
}

impl<'a> ToSafe<SafeIncompleteProduct<'a>> for IncompleteProduct<'a> {
    fn verify(self, conn: &SqliteConnection) -> Result<SafeIncompleteProduct<'a>> {
        let ctg = Categories::find_by_id(conn, self.category)?;
        if ctg.is_leaf() {
            Ok(SafeIncompleteProduct {
                category: self.category,
                prodname: self.prodname,
                price: self.price as i64,
                quantity: self.quantity.get() as i64,
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
        let id_cloned = Uuid::new_v4();
        let shortid_str = id_cloned.as_fields().0.to_string();
        let id_cloned = id_cloned.to_string();
        let value = (
            id.eq(&id_cloned),
            shortid.eq(&shortid_str),
            seller_id.eq(seller.get_id()),
            category.eq(self.category),
            prodname.eq(self.prodname),
            price.eq(self.price),
            quantity.eq(self.quantity),
            description.eq(self.description),
            product_status.eq(ProductStatus::default()),
        );
        diesel::insert_into(products).values(value).execute(conn)?;
        Ok(ProductId { id: id_cloned })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, FromForm)]
pub struct IncompleteProduct<'a> {
    pub category: &'a str,
    pub prodname: &'a str,
    pub price: u32,
    pub quantity: NonZeroU32,
    pub description: &'a str,
}

impl<'a> IncompleteProduct<'a> {
    pub fn new(
        category: &'a LeafCategory,
        prodname: &'a str,
        price: u32,
        quantity: u32,
        description: &'a str,
    ) -> Result<Self> {
        let quantity = NonZeroU32::new(quantity).ok_or(SailsDbError::IllegalPriceOrQuantity)?;
        Ok(Self {
            category: category.id(),
            prodname,
            price,
            quantity,
            description,
        })
    }

    pub fn create(self, conn: &SqliteConnection, seller: &UserId) -> Result<ProductId> {
        self.verify(conn)?.create(conn, seller)
    }
}

/// A single product info entry, corresponding to a row in the table `products`.
#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone,
)]
#[table_name = "products"]
pub struct ProductInfo {
    id: String,
    shortid: String,
    seller_id: String,
    category: String,
    prodname: String,
    price: i64,
    quantity: i64,
    description: String,
    product_status: ProductStatus,
}

impl ProductInfo {
    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<ProductInfo>(conn)?)
    }

    pub fn to_id(&self) -> ProductId {
        ProductId {
            id: self.id.clone(),
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

    pub fn get_category_id(&self) -> &str {
        &self.category
    }

    pub fn get_prodname(&self) -> &str {
        &self.prodname
    }

    pub fn get_price(&self) -> u32 {
        self.price as u32
    }

    pub fn get_quantity(&self) -> u32 {
        self.quantity as u32
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
    pub fn set_price(mut self, price: u32) -> Self {
        self.price = price as i64;
        self
    }

    /// Set the product info's quantity.
    // Quantity is not allowed to be set to zero but only allowed to be add/sub to zero.
    // Practically, we cannot add to zero though.
    pub fn set_quantity(mut self, qty: u32) -> Result<Self> {
        let qty = NonZeroU32::new(qty).ok_or(SailsDbError::IllegalPriceOrQuantity)?;
        self.quantity = qty.get() as i64;
        Ok(self)
    }

    pub(crate) fn sub_quantity(mut self, qty: u32) -> Result<Self> {
        if let Some(u) = (self.quantity as u32).checked_sub(qty) {
            self.quantity = u as i64;
            // If quantity gets to zero, we disable the product.
            if self.quantity == 0 {
                Ok(self.set_product_status(ProductStatus::Disabled))
            } else {
                Ok(self)
            }
        } else {
            Err(SailsDbError::Overflow)
        }
    }

    pub(crate) fn add_quantity(mut self, qty: u32) -> Result<Self> {
        if let Some(u) = (self.quantity as u32).checked_add(qty) {
            self.quantity = u as i64;
            // If quantity higher than zero, we reactivate the product.
            if self.quantity > 0 {
                Ok(self.set_product_status(ProductStatus::Verified))
            } else {
                Ok(self)
            }
        } else {
            Err(SailsDbError::Overflow)
        }
    }

    /// Set the product info's description.
    pub fn set_description(mut self, description: impl ToString) -> Self {
        self.description = description.to_string();
        self
    }

    /// Get a reference to the product info's shortid.
    pub fn get_shortid(&self) -> &str {
        &self.shortid
    }

    /// Get a reference to the product info's product status.
    pub fn get_product_status(&self) -> &ProductStatus {
        &self.product_status
    }

    /// Set the product info's product status.
    // extern crate are not allowed to manually set the product status to sold. Otherwise, the transactions and the products are not gonna agree.
    pub fn set_product_status(mut self, product_status: ProductStatus) -> Self {
        self.product_status = product_status;
        self
    }

    pub fn readable(&self, conn: &SqliteConnection, user: &UserId) -> Result<bool> {
        Ok(if self.seller_id == user.get_id() {
            user.get_info(conn)?
                .get_user_status()
                .contains(UserStatus::PROD_SELF_READABLE)
        } else {
            user.get_info(conn)?
                .get_user_status()
                .contains(UserStatus::PROD_OTHERS_READABLE)
        })
    }

    pub fn writable(&self, conn: &SqliteConnection, user: &UserId) -> Result<bool> {
        Ok(if self.seller_id == user.get_id() {
            user.get_info(conn)?
                .get_user_status()
                .contains(UserStatus::PROD_SELF_WRITABLE)
        } else {
            user.get_info(conn)?
                .get_user_status()
                .contains(UserStatus::PROD_OTHERS_WRITABLE)
        })
    }

    pub fn removable(&self, conn: &SqliteConnection, user: &UserId) -> Result<bool> {
        Ok(if self.seller_id == user.get_id() {
            user.get_info(conn)?
                .get_user_status()
                .contains(UserStatus::PROD_SELF_REMOVABLE)
        } else {
            user.get_info(conn)?
                .get_user_status()
                .contains(UserStatus::PROD_OTHERS_REMOVABLE)
        })
    }
}

#[cfg(test)]
mod tests;
