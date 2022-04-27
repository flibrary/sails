use crate::{
    enums::UserStatus,
    error::{SailsDbError, SailsDbResult as Result},
    products::{ProductFinder, ProductId},
    schema::{digiconmappings, digicons},
    transactions::TransactionFinder,
    users::UserId,
};
use diesel::{dsl::count, prelude::*, sqlite::Sqlite};
use rocket::FromForm;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use uuid::Uuid;

// A pseudo struct for managing the digicons table.
pub struct Digicons;

impl Digicons {
    pub fn list_all(conn: &SqliteConnection) -> Result<Vec<Digicon>> {
        use crate::schema::digicons::dsl::*;
        Ok(digicons.load::<Digicon>(conn)?)
    }

    pub fn list_all_authorized(conn: &SqliteConnection, user: &UserId) -> Result<Vec<Digicon>> {
        let mut authorized = Vec::new();
        for x in Self::list_all(conn)? {
            if DigiconMappingFinder::is_authorized(conn, user, &x)? {
                authorized.push(x);
            }
        }
        Ok(authorized)
    }

    pub fn find_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<Digicon> {
        use crate::schema::digicons::dsl::*;
        Ok(digicons
            .into_boxed()
            .filter(id.eq(id_provided))
            .first::<Digicon>(conn)?)
    }

    /// Note: this returns the first category matching the name.
    /// Name is NOT guaranteed to be unique. Whenever possible, use find_by_id instead.
    pub fn find_by_link(conn: &SqliteConnection, link_provided: &str) -> Result<Digicon> {
        use crate::schema::digicons::dsl::*;
        Ok(digicons
            .into_boxed()
            .filter(link.eq(link_provided))
            .first::<Digicon>(conn)?)
    }

    pub fn delete_all(conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::digicons::dsl::*;
        Ok(diesel::delete(digicons).execute(conn)?)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Value {
    pub name: Arc<str>,
    pub link: Arc<str>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DigiconsBuilder {
    #[serde(rename = "digicons")]
    inner: HashMap<Arc<str>, Value>,
}

impl DigiconsBuilder {
    pub fn new(inner: HashMap<Arc<str>, Value>) -> Self {
        Self { inner }
    }

    pub fn build(self, conn: &SqliteConnection) -> Result<()> {
        for (id, value) in self.inner {
            Digicon::create(conn, id, value.name, value.link)?;
        }
        Ok(())
    }
}

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "digicons"]
pub struct Digicon {
    id: String,
    name: String,
    link: String,
}

impl Digicon {
    pub fn new(id: impl ToString, name: impl ToString, link: impl ToString) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            link: link.to_string(),
        }
    }

    // Create a new digicon with a specific ID
    pub fn create(
        conn: &SqliteConnection,
        id_provided: impl ToString,
        name_provided: impl ToString,
        link_provided: impl ToString,
    ) -> Result<Self> {
        use crate::schema::digicons::dsl::*;
        let digicon = Digicon::new(id_provided, name_provided, link_provided);

        if let Ok(0) = digicons
            .filter(id.eq(digicon.get_id()))
            .count()
            .get_result(conn)
        {
            // This means that we have to insert
            diesel::insert_into(digicons)
                .values(&digicon)
                .execute(conn)?
        } else {
            return Err(SailsDbError::DigiconExisted);
        };
        Ok(digicon)
    }

    pub fn delete(self, conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::digicons::dsl::*;
        Ok(diesel::delete(digicons.filter(id.eq(self.id))).execute(conn)?)
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<Digicon>(conn)?)
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_link(&self) -> &str {
        &self.link
    }

    pub fn set_link(mut self, link: impl ToString) -> Self {
        self.link = link.to_string();
        self
    }

    pub fn set_name(mut self, name: impl ToString) -> Self {
        self.name = name.to_string();
        self
    }
}

type BoxedQuery<'a> = digiconmappings::BoxedQuery<'a, Sqlite, digiconmappings::SqlType>;

/// A search query helper (builder)
pub struct DigiconMappingFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

impl<'a> DigiconMappingFinder<'a> {
    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<DigiconMapping>> {
        Self::new(conn, None).search()
    }

    pub fn search(self) -> Result<Vec<DigiconMapping>> {
        Ok(self.query.load::<DigiconMapping>(self.conn)?)
    }

    pub fn search_digicon(self) -> Result<Vec<Digicon>> {
        let conn = self.conn;
        self.query
            .load::<DigiconMapping>(conn)?
            .into_iter()
            .map(|x| Digicons::find_by_id(conn, x.get_digicon()))
            .collect()
    }

    pub fn first(self) -> Result<DigiconMapping> {
        Ok(self.query.first::<DigiconMapping>(self.conn)?)
    }

    pub fn delete_by_product(self, product_id: &'a ProductId) -> Result<()> {
        use crate::schema::digiconmappings::dsl::*;
        diesel::delete(digiconmappings.filter(product.eq(product_id.get_id())))
            .execute(self.conn)?;
        Ok(())
    }

    pub fn id(mut self, id_provided: &'a str) -> Self {
        use crate::schema::digiconmappings::dsl::*;
        self.query = self.query.filter(id.eq(id_provided));
        self
    }

    pub fn product(mut self, product_id: &'a ProductId) -> Self {
        use crate::schema::digiconmappings::dsl::*;
        self.query = self.query.filter(product.eq(product_id.get_id()));
        self
    }

    pub fn digicon(mut self, digicon_id: &'a Digicon) -> Self {
        use crate::schema::digiconmappings::dsl::*;
        self.query = self.query.filter(digicon.eq(digicon_id.get_id()));
        self
    }

    pub fn count(self) -> Result<i64> {
        use crate::schema::digiconmappings::dsl::*;
        Ok(self.query.select(count(id)).first::<i64>(self.conn)?)
    }

    pub fn has_mapping(
        conn: &'a SqliteConnection,
        digicon: &'a Digicon,
        product: &'a ProductId,
    ) -> Result<bool> {
        Ok(Self::new(conn, None)
            .digicon(digicon)
            .product(product)
            .count()?
            > 0)
    }

    pub fn is_authorized(
        conn: &'a SqliteConnection,
        user: &'a UserId,
        digicon: &'a Digicon,
    ) -> Result<bool> {
        // Admin who can manage the digicons gets to access all digicons
        if user
            .get_info(conn)?
            .get_user_status()
            .contains(UserStatus::DIGICON_WRITABLE)
        {
            return Ok(true);
        }

        let mut bought_products = TransactionFinder::new(conn, None)
            .buyer(user)
            // Products with digicons don't have status paid
            // Only effective orders count and we don't need to care about duplication as HashSet takes care after it.
            .status(crate::enums::TransactionStatus::Finished, crate::Cmp::Equal)
            .search_info()?
            .into_iter()
            .map(|t| t.get_product().to_string())
            .collect::<HashSet<String>>();
        let owned_products = ProductFinder::new(conn, None)
            .seller(user)
            .search()?
            .into_iter()
            .map(|x| x.get_id().to_string())
            .collect::<HashSet<String>>();
        let mapped_products = Self::new(conn, None)
            .digicon(digicon)
            .search()?
            .into_iter()
            .map(|x| x.get_product().to_string())
            .collect::<HashSet<String>>();

        bought_products = owned_products.union(&bought_products).cloned().collect();
        // If the user owned or bought the product which contains the digicon, he is allowed to access it
        Ok(bought_products.intersection(&mapped_products).count() > 0)
    }

    pub fn new(conn: &'a SqliteConnection, query: Option<BoxedQuery<'a>>) -> Self {
        use crate::schema::digiconmappings::dsl::*;
        if let Some(q) = query {
            Self { conn, query: q }
        } else {
            Self {
                conn,
                query: digiconmappings.into_boxed(),
            }
        }
    }
}

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "digiconmappings"]
pub struct DigiconMapping {
    id: String,
    digicon: String,
    product: String,
}

impl DigiconMapping {
    pub fn create(
        conn: &SqliteConnection,
        digicon_p: &Digicon,
        product_p: &ProductId,
    ) -> Result<Self> {
        // Only create digicon mapping if we have not done so.
        if !DigiconMappingFinder::has_mapping(conn, digicon_p, product_p)? {
            use crate::schema::digiconmappings::dsl::*;
            let digiconmapping = Self {
                id: Uuid::new_v4().to_string(),
                digicon: digicon_p.get_id().to_string(),
                product: product_p.get_id().to_string(),
            };
            diesel::insert_into(digiconmappings)
                .values(&digiconmapping)
                .execute(conn)?;
            // There should be one mapping now
            assert!(DigiconMappingFinder::has_mapping(
                conn, digicon_p, product_p
            )?);
            Ok(digiconmapping)
        } else {
            Err(SailsDbError::DigiconMappingExisted)
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_digicon(&self) -> &str {
        &self.digicon
    }

    pub fn get_product(&self) -> &str {
        &self.product
    }

    pub fn delete(self, conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::digiconmappings::dsl::*;
        Ok(diesel::delete(digiconmappings.filter(id.eq(self.id))).execute(conn)?)
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<DigiconMapping>(conn)?)
    }
}
