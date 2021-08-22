use crate::{
    enums::{ProductStatus, TransactionStatus},
    error::{SailsDbError, SailsDbResult as Result},
    products::{ProductFinder, ProductId, ToSafe},
    schema::transactions,
    users::UserId,
    Cmp, Order,
};
use chrono::naive::NaiveDateTime;
use diesel::{prelude::*, sqlite::Sqlite};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// A psuedo struct for managing transactions
pub struct Transactions;

impl Transactions {
    pub fn buy(
        conn: &SqliteConnection,
        product_p: &ProductId,
        buyer_p: &UserId,
    ) -> Result<TransactionId> {
        use crate::schema::transactions::dsl::*;

        let product_info = product_p.get_info(conn)?;

        if product_info.get_product_status() == &ProductStatus::Verified {
            let id_cloned = Uuid::new_v4();
            let shortid_str = id_cloned.as_fields().0.to_string();
            let tx = TransactionInfo {
                id: id_cloned.to_string(),
                shortid: shortid_str,
                seller: product_info.get_seller_id().to_string(),
                product: product_p.get_id().to_string(),
                buyer: buyer_p.get_id().to_string(),
                time_sent: chrono::offset::Local::now().naive_utc(),
                transaction_status: TransactionStatus::Placed,
            };

            // Create transaction record
            diesel::insert_into(transactions).values(tx).execute(conn)?;

            // Change the product status to sold
            product_info.verify(conn)?.set_sold().update(conn)?;

            // Return the transaction ID
            Ok(TransactionId {
                id: id_cloned.to_string(),
            })
        } else {
            Err(SailsDbError::OrderOnUnverified)
        }
    }

    pub fn buyer_refundable(conn: &SqliteConnection, buyer: &UserId) -> Result<bool> {
        Ok(TransactionFinder::new(conn, None)
            .buyer(buyer.get_id())
            .status(TransactionStatus::Refunded, Cmp::Equal)
            .search()?
            .len()
            > 2)
    }
}

// The ID referencing a single transaction
#[derive(Debug, Serialize, Deserialize, Identifiable, Queryable, Clone)]
#[table_name = "transactions"]
pub struct TransactionId {
    id: String,
}

impl TransactionId {
    pub fn to_uuid(&self) -> Result<Uuid> {
        Ok(<Uuid as std::str::FromStr>::from_str(&self.id)?)
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_info(&self, conn: &SqliteConnection) -> Result<TransactionInfo> {
        use crate::schema::transactions::dsl::*;
        Ok(transactions
            .filter(id.eq(&self.id))
            .first::<TransactionInfo>(conn)?)
    }

    pub fn refund(&self, conn: &SqliteConnection) -> Result<()> {
        let info = self.get_info(conn)?;
        ProductFinder::new(conn, None)
            .id(info.get_product())
            .first_info()?
            .set_verified()
            .update(conn)?;
        info.set_transaction_status(TransactionStatus::Refunded)
            .update(conn)
            .map(|_| ())
    }
}

/// A single transaction info entry, corresponding to a row in the table `transactions`
#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone,
)]
#[table_name = "transactions"]
pub struct TransactionInfo {
    id: String,
    shortid: String,
    seller: String,
    product: String,
    buyer: String,
    time_sent: NaiveDateTime,
    transaction_status: TransactionStatus,
}

impl TransactionInfo {
    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<TransactionInfo>(conn)?)
    }

    /// Get a reference to the transaction info's id.
    pub fn get_id(&self) -> &str {
        &self.id
    }

    /// Get a reference to the transaction info's shortid.
    pub fn get_shortid(&self) -> &str {
        &self.shortid
    }

    /// Get a reference to the transaction info's product.
    pub fn get_product(&self) -> &str {
        &self.product
    }

    /// Get a reference to the transaction info's buyer.
    pub fn get_buyer(&self) -> &str {
        &self.buyer
    }

    /// Get a reference to the transaction info's time sent.
    pub fn get_time_sent(&self) -> &NaiveDateTime {
        &self.time_sent
    }

    /// Get a reference to the transaction info's transaction status.
    pub fn get_transaction_status(&self) -> &TransactionStatus {
        &self.transaction_status
    }

    /// Set the transaction info's transaction status.
    pub fn set_transaction_status(mut self, transaction_status: TransactionStatus) -> Self {
        self.transaction_status = transaction_status;
        self
    }

    /// Get a reference to the transaction info's seller.
    pub fn seller(&self) -> &String {
        &self.seller
    }
}

type BoxedQuery<'a> = transactions::BoxedQuery<'a, Sqlite, transactions::SqlType>;

/// A search query helper (builder)
pub struct TransactionFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

impl<'a> TransactionFinder<'a> {
    pub fn list_info(conn: &'a SqliteConnection) -> Result<Vec<TransactionInfo>> {
        Self::new(conn, None).search_info()
    }

    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<TransactionId>> {
        Self::new(conn, None).search()
    }

    pub fn new(conn: &'a SqliteConnection, query: Option<BoxedQuery<'a>>) -> Self {
        use crate::schema::transactions::dsl::*;
        if let Some(q) = query {
            Self { conn, query: q }
        } else {
            Self {
                conn,
                query: transactions.into_boxed(),
            }
        }
    }

    pub fn search(self) -> Result<Vec<TransactionId>> {
        use crate::schema::transactions::dsl::*;
        Ok(self
            .query
            .select(id)
            .load::<String>(self.conn)?
            .into_iter()
            .map(|x| TransactionId { id: x })
            .collect())
    }

    pub fn search_info(self) -> Result<Vec<TransactionInfo>> {
        Ok(self.query.load::<TransactionInfo>(self.conn)?)
    }

    pub fn first(self) -> Result<TransactionId> {
        use crate::schema::transactions::dsl::*;
        Ok(TransactionId {
            id: self.query.select(id).first::<String>(self.conn)?,
        })
    }

    pub fn first_info(self) -> Result<TransactionInfo> {
        Ok(self.query.first::<TransactionInfo>(self.conn)?)
    }

    pub fn id(mut self, id_provided: &'a str) -> Self {
        use crate::schema::transactions::dsl::*;
        self.query = self.query.filter(id.eq(id_provided));
        self
    }

    pub fn seller(mut self, seller_id: &'a str) -> Self {
        use crate::schema::transactions::dsl::*;
        self.query = self.query.filter(seller.eq(seller_id));
        self
    }

    pub fn product(mut self, product_id: &'a str) -> Self {
        use crate::schema::transactions::dsl::*;
        self.query = self.query.filter(product.eq(product_id));
        self
    }

    pub fn buyer(mut self, buyer_id: &'a str) -> Self {
        use crate::schema::transactions::dsl::*;
        self.query = self.query.filter(buyer.eq(buyer_id));
        self
    }

    pub fn time(mut self, time_provided: NaiveDateTime, cmp: Cmp) -> Self {
        use crate::schema::transactions::dsl::*;
        match cmp {
            Cmp::GreaterThan => self.query = self.query.filter(time_sent.gt(time_provided)),
            Cmp::LessThan => self.query = self.query.filter(time_sent.lt(time_provided)),
            Cmp::GreaterEqual => self.query = self.query.filter(time_sent.ge(time_provided)),
            Cmp::LessEqual => self.query = self.query.filter(time_sent.le(time_provided)),
            Cmp::NotEqual => self.query = self.query.filter(time_sent.ne(time_provided)),
            Cmp::Equal => self.query = self.query.filter(time_sent.eq(time_provided)),
        }
        self
    }

    pub fn order_by_time(mut self, order: Order) -> Self {
        use crate::schema::transactions::dsl::*;
        match order {
            Order::Asc => self.query = self.query.order(time_sent.asc()),
            Order::Desc => self.query = self.query.order(time_sent.desc()),
        }
        self
    }

    pub fn status(mut self, status: TransactionStatus, cmp: Cmp) -> Self {
        use crate::schema::transactions::dsl::*;
        match cmp {
            Cmp::Equal => self.query = self.query.filter(transaction_status.eq(status)),
            Cmp::NotEqual => self.query = self.query.filter(transaction_status.ne(status)),
            // Currently it makes no sense for us to do so
            _ => unimplemented!(),
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        categories::Category, products::IncompleteProduct, test_utils::establish_connection,
        users::*,
    };

    #[test]
    fn create_transaction() {
        let conn = establish_connection();
        // our seller
        let seller = UserForm::new(
            "TestUser@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        let buyer = UserForm::new(
            "AtypicalBuyer@example.org",
            "NFLS",
            "+86 18353232340",
            "strongpasswd",
            None,
        )
        .to_ref()
        .unwrap()
        .create(&conn)
        .unwrap();

        // The book category
        let econ = Category::create(&conn, "Economics Books")
            .and_then(Category::into_leaf)
            .unwrap();
        let book_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            700,
            "A very great book on the subject of Economics",
        )
        .create(&conn, &seller)
        .unwrap();

        // Unverified products are not subjected to purchases.
        assert!(Transactions::buy(&conn, &book_id, &buyer).is_err());

        // Verify the book
        book_id
            .get_info(&conn)
            .unwrap()
            .verify(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        // Purchase it
        let tx_id = Transactions::buy(&conn, &book_id, &buyer).unwrap();

        // There should be only one transaction entry
        assert_eq!(TransactionFinder::list(&conn).unwrap().len(), 1);

        // The book status should be sold now
        assert_eq!(
            book_id.get_info(&conn).unwrap().get_product_status(),
            &ProductStatus::Sold
        );

        // The book is locked and cannot be changed
        assert!(book_id
            .update(
                &conn,
                IncompleteProduct::new(
                    &econ,
                    "Agenuine Economics book",
                    600,
                    "That is a bad book though",
                )
                .verify(&conn)
                .unwrap()
            )
            .is_err());

        // Refund the book, returning the book to verfied state
        tx_id.refund(&conn).unwrap();

        // The book is now verfied but not sold
        assert_eq!(
            book_id.get_info(&conn).unwrap().get_product_status(),
            &ProductStatus::Verified
        );

        // The transaction status should now be refunded
        assert_eq!(
            TransactionFinder::new(&conn, None)
                .status(TransactionStatus::Refunded, Cmp::Equal)
                .search()
                .unwrap()
                .len(),
            1
        );

        // ... and the book can be updated now
        assert!(book_id
            .update(
                &conn,
                IncompleteProduct::new(
                    &econ,
                    "Agenuine Economics book",
                    600,
                    "That is a bad book though",
                )
                .verify(&conn)
                .unwrap()
            )
            .is_ok());
    }
}
