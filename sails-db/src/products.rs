use super::users::User;
use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    schema::products,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::num::NonZeroI64;
use uuid::Uuid;

/// A pseudo products used to manage table `products`
pub struct Products;

impl Products {
    // CRUD: READ
    pub fn list(conn: &SqliteConnection) -> Result<Vec<Product>> {
        use crate::schema::products::dsl::*;
        Ok(products.load::<Product>(conn)?)
    }

    // CRUD: READ
    pub fn find_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<Product> {
        use crate::schema::products::dsl::*;
        Ok(products.filter(id.eq(id_provided)).first::<Product>(conn)?)
    }

    pub fn search_by_price(
        conn: &SqliteConnection,
        prodname_p: &str,
        price_p: NonZeroI64,
        asc: bool,
        gt: bool,
    ) -> Result<Vec<Product>> {
        use crate::schema::products::dsl::*;
        Ok(match (asc, gt) {
            (true, true) => products
                .filter(prodname.eq(prodname_p))
                .filter(price.gt(price_p.get()))
                .order(price.asc())
                .load::<Product>(conn)?,
            (true, false) => products
                .filter(prodname.eq(prodname_p))
                .filter(price.lt(price_p.get()))
                .order(price.asc())
                .load::<Product>(conn)?,
            (false, true) => products
                .filter(prodname.eq(prodname_p))
                .filter(price.gt(price_p.get()))
                .order(price.desc())
                .load::<Product>(conn)?,
            (false, false) => products
                .filter(prodname.eq(prodname_p))
                .filter(price.lt(price_p.get()))
                .order(price.desc())
                .load::<Product>(conn)?,
        })
    }

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
mod tests {
    use std::num::NonZeroI64;

    use super::Products;
    use crate::{
        test_utils::establish_connection,
        users::{User, Users},
    };

    #[test]
    fn create_product() {
        let conn = establish_connection();
        // our seller
        let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
        Users::create_or_update(&conn, user.clone()).unwrap();
        Products::create_product(
            &conn,
            &user,
            "Krugman's Economics 2nd Edition",
            NonZeroI64::new(700).unwrap(),
            "A very great book on the subject of Economics",
        )
        .unwrap();
        assert_eq!(Products::list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn search_products() {
        let conn = establish_connection();
        // our seller
        let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
        Users::create_or_update(&conn, user.clone()).unwrap();
        Products::create_product(
            &conn,
            &user,
            "Krugman's Economics 2nd Edition",
            NonZeroI64::new(700).unwrap(),
            "A very great book on the subject of Economics",
        )
        .unwrap();

        // Another Krugman's Economics, with a lower price!
        Products::create_product(
            &conn,
            &user,
            "Krugman's Economics 2nd Edition",
            NonZeroI64::new(500).unwrap(),
            "A very great book on the subject of Economics",
        )
        .unwrap();

        // Another Krugman's Economics, with a lower price!
        Products::create_product(
            &conn,
            &user,
            "Krugman's Economics 2nd Edition",
            NonZeroI64::new(600).unwrap(),
            "That is a bad book though",
        )
        .unwrap();

        // Feynman's Lecture on Physics!
        Products::create_product(
            &conn,
            &user,
            "Feynman's Lecture on Physics",
            NonZeroI64::new(900).unwrap(),
            "A very masterpiece on the theory of the universe",
        )
        .unwrap();

        // Search lower than CNY 300 Feynman's Lecture on Physics
        assert_eq!(
            Products::search_by_price(
                &conn,
                "Feynman's Lecture on Physics",
                NonZeroI64::new(300).unwrap(),
                true,
                false
            )
            .unwrap()
            .len(),
            0
        );

        // Search higher than CNY 300 Feynman's Lecture on Physics
        assert_eq!(
            Products::search_by_price(
                &conn,
                "Feynman's Lecture on Physics",
                NonZeroI64::new(300).unwrap(),
                true,
                true
            )
            .unwrap()
            .len(),
            1
        );

        // Krugman
        assert_eq!(
            Products::search_by_price(
                &conn,
                "Krugman's Economics 2nd Edition",
                NonZeroI64::new(550).unwrap(),
                true,
                true
            )
            .unwrap()
            .len(),
            2
        );
    }

    #[test]
    fn delete_product() {
        let conn = establish_connection();
        // our seller
        let user = User::new("TestUser", None, "NFLS", "+86 18353232340", "strongpasswd").unwrap();
        Users::create_or_update(&conn, user.clone()).unwrap();
        let id = Products::create_product(
            &conn,
            &user,
            "Krugman's Economics 2nd Edition",
            NonZeroI64::new(700).unwrap(),
            "A very great book on the subject of Economics",
        )
        .unwrap();
        assert_eq!(Products::list(&conn).unwrap().len(), 1);
        Products::delete_by_id(&conn, &id).unwrap();
        assert_eq!(Products::list(&conn).unwrap().len(), 0);
    }
}
