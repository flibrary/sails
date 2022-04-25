use std::num::NonZeroU32;

use crate::{
    enums::{ProductStatus, TransactionStatus},
    error::{SailsDbError, SailsDbResult as Result},
    products::{ProductFinder, ProductId},
    schema::transactions,
    users::UserId,
    Cmp, Order,
};
use chrono::naive::NaiveDateTime;
use diesel::{dsl::count, prelude::*, sqlite::Sqlite};
use num_bigint::{BigUint, ToBigUint};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// A psuedo struct for managing transactions
pub struct Transactions;

impl Transactions {
    pub fn buy(
        conn: &SqliteConnection,
        product_p: &ProductId,
        buyer_p: &UserId,
        qty: u32,
        addr: impl ToString,
    ) -> Result<TransactionId> {
        let qty = NonZeroU32::new(qty).ok_or(SailsDbError::IllegalPriceOrQuantity)?;

        use crate::schema::transactions::dsl::*;

        let product_info = product_p.get_info(conn)?;

        // Seller should be able to purchase their own products
        if product_info.get_seller_id() == buyer_p.get_id() {
            return Err(SailsDbError::SelfPurchaseNotAllowed);
        }

        if product_info.get_product_status() == &ProductStatus::Verified {
            let id_cloned = Uuid::new_v4();
            let shortid_str = id_cloned.as_fields().0.to_string();
            let tx = TransactionInfo {
                id: id_cloned.to_string(),
                shortid: shortid_str,
                seller: product_info.get_seller_id().to_string(),
                product: product_p.get_id().to_string(),
                price: product_info.get_price() as i64,
                quantity: qty.get() as i64,
                address: addr.to_string(),
                buyer: buyer_p.get_id().to_string(),
                time_sent: chrono::offset::Local::now().naive_utc(),
                transaction_status: if product_info.get_price() != 0 {
                    TransactionStatus::Placed
                } else {
                    // If the product is free, we just finish the transaction
                    TransactionStatus::Finished
                },
            };

            // Create transaction record
            diesel::insert_into(transactions).values(tx).execute(conn)?;

            // Sub product quantity
            // IMPORTANT We shall not use ? operator here because that abort the function and doesn't clean up
            if product_info
                .sub_quantity(qty.get())
                .map(|s| s.update(conn))
                .is_ok()
            {
                // Return the transaction ID
                Ok(TransactionId {
                    id: id_cloned.to_string(),
                })
            } else {
                // There are some issues changing the status of the book, and we shall delete the transaction
                diesel::delete(transactions.filter(id.eq(&id_cloned.to_string()))).execute(conn)?;
                Err(SailsDbError::FailedAlterProductQuantity)
            }
        } else {
            Err(SailsDbError::OrderOnUnverified)
        }
    }

    pub fn buyer_refundable(conn: &SqliteConnection, buyer: &UserId) -> Result<bool> {
        Ok(TransactionFinder::new(conn, None)
            .buyer(buyer)
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
        self.get_info(conn)?.refund(conn)
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
    price: i64,
    quantity: i64,
    address: String,
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

    pub fn refund(&self, conn: &SqliteConnection) -> Result<()> {
        // Return the products to `verified` state.
        ProductFinder::new(conn, None)
            .id(self.get_product())
            .first_info()?
            .add_quantity(self.quantity as u32)?
            .update(conn)?;
        self.clone()
            .set_transaction_status(TransactionStatus::Refunded)
            .update(conn)
            .map(|_| ())
    }

    /// Get a reference to the transaction info's shortid.
    pub fn get_shortid(&self) -> &str {
        &self.shortid
    }

    /// Get a reference to the transaction info's price.
    pub fn get_price(&self) -> u32 {
        self.price as u32
    }

    /// Get a reference to the transaction info's quantity.
    pub fn get_quantity(&self) -> u32 {
        self.quantity as u32
    }

    pub fn get_total(&self) -> BigUint {
        let qty: BigUint = self.get_quantity().into();
        let price: BigUint = self.get_price().into();
        qty * price
    }

    /// Get a reference to the transaction info's product.
    pub fn get_product(&self) -> &str {
        &self.product
    }

    /// Get a reference to the transaction info's buyer.
    pub fn get_buyer(&self) -> &str {
        &self.buyer
    }

    pub fn get_address(&self) -> &str {
        &self.address
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
    pub fn get_seller(&self) -> &str {
        self.seller.as_str()
    }
}

type BoxedQuery<'a> = transactions::BoxedQuery<'a, Sqlite, transactions::SqlType>;

/// A search query helper (builder)
pub struct TransactionFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

#[derive(Eq, PartialEq, Debug, Default)]
pub struct TxStats {
    pub placed_subtotal: BigUint,
    pub paid_subtotal: BigUint,
    pub finished_subtotal: BigUint,
    pub refunded_subtotal: BigUint,
    pub total: BigUint, // including placed, paid, and finished
    pub placed: BigUint,
    pub paid: BigUint,
    pub refunded: BigUint,
    pub finished: BigUint,
    pub total_num: BigUint,
}

impl<'a> TransactionFinder<'a> {
    pub fn list_info(conn: &'a SqliteConnection) -> Result<Vec<TransactionInfo>> {
        Self::new(conn, None).search_info()
    }

    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<TransactionId>> {
        Self::new(conn, None).search()
    }

    pub fn count(self) -> Result<BigUint> {
        use crate::schema::transactions::dsl::*;
        Ok(self
            .query
            .select(count(id))
            .first::<i64>(self.conn)?
            .to_biguint()
            .unwrap()) // guranteed to be positive.
    }

    pub fn most_recent_order(
        conn: &'a SqliteConnection,
        user: &'a UserId,
    ) -> Result<TransactionInfo> {
        Self::new(conn, None)
            .buyer(user)
            .order_by_time(Order::Desc)
            .first_info()
    }

    pub fn stats(conn: &'a SqliteConnection, user: Option<&'a UserId>) -> Result<TxStats> {
        // TODO: we should write SQL instead
        fn get_total(finder: TransactionFinder) -> Result<BigUint> {
            Ok(finder.search_info()?.iter().map(|x| x.get_total()).sum())
        }

        fn selection<'a>(
            user: &Option<&'a UserId>,
            conn: &'a SqliteConnection,
        ) -> TransactionFinder<'a> {
            if let Some(user) = user {
                TransactionFinder::new(conn, None).seller(user)
            } else {
                TransactionFinder::new(conn, None)
            }
        }

        let placed_subtotal =
            get_total(selection(&user, conn).status(TransactionStatus::Placed, Cmp::Equal))?;

        let paid_subtotal =
            get_total(selection(&user, conn).status(TransactionStatus::Paid, Cmp::Equal))?;

        let refunded_subtotal =
            get_total(selection(&user, conn).status(TransactionStatus::Refunded, Cmp::Equal))?;

        let finished_subtotal =
            get_total(selection(&user, conn).status(TransactionStatus::Finished, Cmp::Equal))?;

        let refunded = selection(&user, conn)
            .status(TransactionStatus::Refunded, Cmp::Equal)
            .count()?;
        let placed = selection(&user, conn)
            .status(TransactionStatus::Placed, Cmp::Equal)
            .count()?;
        let paid = selection(&user, conn)
            .status(TransactionStatus::Paid, Cmp::Equal)
            .count()?;
        let finished = selection(&user, conn)
            .status(TransactionStatus::Finished, Cmp::Equal)
            .count()?;

        Ok(TxStats {
            total_num: placed.clone() + paid.clone() + finished.clone(),
            total: placed_subtotal.clone() + paid_subtotal.clone() + finished_subtotal.clone(),
            placed_subtotal,
            paid_subtotal,
            refunded_subtotal,
            finished_subtotal,
            placed,
            paid,
            refunded,
            finished,
        })
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

    pub fn seller(mut self, seller_id: &'a UserId) -> Self {
        use crate::schema::transactions::dsl::*;
        self.query = self.query.filter(seller.eq(seller_id.get_id()));
        self
    }

    pub fn product(mut self, product_id: &'a ProductId) -> Self {
        use crate::schema::transactions::dsl::*;
        self.query = self.query.filter(product.eq(product_id.get_id()));
        self
    }

    pub fn buyer(mut self, buyer_id: &'a UserId) -> Self {
        use crate::schema::transactions::dsl::*;
        self.query = self.query.filter(buyer.eq(buyer_id.get_id()));
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
        categories::{Category, CtgTrait},
        products::{IncompleteProduct, ToSafe},
        test_utils::establish_connection,
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
        let econ = Category::create(&conn, "Economics Books", 490)
            .and_then(Category::into_leaf)
            .unwrap();
        let book_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            700,
            1,
            "A very great book on the subject of Economics",
        )
        .unwrap()
        .create(&conn, &seller)
        .unwrap();

        // Unverified products are not subjected to purchases.
        assert!(matches!(
            Transactions::buy(
                &conn,
                &book_id,
                &buyer,
                1,
                "258 Huanhu South Road, Dongqian Lake, Ningbo, China"
            )
            .err()
            .unwrap(),
            SailsDbError::OrderOnUnverified
        ));

        // Verify the book
        book_id
            .get_info(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        // We cannot purchase more than available.
        assert!(matches!(
            Transactions::buy(
                &conn,
                &book_id,
                &buyer,
                2,
                "258 Huanhu South Road, Dongqian Lake, Ningbo, China"
            )
            .err()
            .unwrap(),
            SailsDbError::FailedAlterProductQuantity
        ));

        // There should be no transaction entry
        assert_eq!(TransactionFinder::list(&conn).unwrap().len(), 0);

        // Purchase it
        let tx_id = Transactions::buy(
            &conn,
            &book_id,
            &buyer,
            1,
            "258 Huanhu South Road, Dongqian Lake, Ningbo, China",
        )
        .unwrap();

        // There should be only one transaction entry
        assert_eq!(TransactionFinder::list(&conn).unwrap().len(), 1);

        // The book status should be disabled now
        assert_eq!(
            book_id.get_info(&conn).unwrap().get_product_status(),
            &ProductStatus::Disabled
        );

        // ... and changing the price should not affect our already-placed order.
        assert!(book_id
            .update(
                &conn,
                IncompleteProduct::new(
                    &econ,
                    "Agenuine Economics book",
                    600,
                    2,
                    "That is a bad book though",
                )
                .unwrap()
                .verify(&conn)
                .unwrap()
            )
            .is_ok());

        // The transaction price should remain unchanged.
        assert_eq!(tx_id.get_info(&conn).unwrap().get_price(), 700);
        assert_eq!(tx_id.get_info(&conn).unwrap().get_quantity(), 1);

        // Refund the book, returning the book to verfied state
        tx_id.refund(&conn).unwrap();

        // The book is now verfied
        assert_eq!(
            book_id.get_info(&conn).unwrap().get_product_status(),
            &ProductStatus::Verified
        );

        // The transaction status should now be refunded
        assert_eq!(
            tx_id.get_info(&conn).unwrap().get_transaction_status(),
            &TransactionStatus::Refunded
        );
    }

    #[test]
    fn txstats() {
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
        let econ = Category::create(&conn, "Economics Books", 490)
            .and_then(Category::into_leaf)
            .unwrap();

        // Placed
        let book_1_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            400,
            1,
            "A very great book on the subject of Economics",
        )
        .unwrap()
        .create(&conn, &seller)
        .unwrap();

        // Placed
        let book_2_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            300,
            1,
            "A very great book on the subject of Economics",
        )
        .unwrap()
        .create(&conn, &seller)
        .unwrap();

        // Paid
        let book_3_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            u32::MAX,
            2,
            "A very great book on the subject of Economics",
        )
        .unwrap()
        .create(&conn, &seller)
        .unwrap();

        // Finished
        let book_4_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            700,
            1,
            "A very great book on the subject of Economics",
        )
        .unwrap()
        .create(&conn, &seller)
        .unwrap();

        // Refunded
        let book_5_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            1000,
            1,
            "A very great book on the subject of Economics",
        )
        .unwrap()
        .create(&conn, &seller)
        .unwrap();

        // Verify the books
        book_1_id
            .get_info(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        book_2_id
            .get_info(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        book_3_id
            .get_info(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        book_4_id
            .get_info(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        book_5_id
            .get_info(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        // No most recent order before purchase
        assert!(TransactionFinder::most_recent_order(&conn, &buyer).is_err());
        // Purchase it
        Transactions::buy(
            &conn,
            &book_1_id,
            &buyer,
            1,
            "258 Huanhu South Road, Dongqian Lake, Ningbo, China",
        )
        .unwrap();
        // Last address updated
        assert_eq!(
            TransactionFinder::most_recent_order(&conn, &buyer)
                .unwrap()
                .get_address(),
            "258 Huanhu South Road, Dongqian Lake, Ningbo, China"
        );

        Transactions::buy(
            &conn,
            &book_2_id,
            &buyer,
            1,
            "258 Huanhu South Road, Dongqian Lake, Ningbo, China",
        )
        .unwrap();
        let tx_3_id =
            Transactions::buy(&conn, &book_3_id, &buyer, 2, "宁波外国语学校 S2202").unwrap();
        // Last address updated
        assert_eq!(
            TransactionFinder::most_recent_order(&conn, &buyer)
                .unwrap()
                .get_address(),
            "宁波外国语学校 S2202"
        );
        let tx_4_id =
            Transactions::buy(&conn, &book_4_id, &buyer, 1, "宁波外国语学校 S2301").unwrap();
        // Last order updated
        assert_eq!(
            TransactionFinder::most_recent_order(&conn, &buyer)
                .unwrap()
                .get_id(),
            tx_4_id.get_id()
        );
        let tx_5_id =
            Transactions::buy(&conn, &book_5_id, &buyer, 1, "宁波市海曙区天一广场").unwrap();
        assert_eq!(
            TransactionFinder::most_recent_order(&conn, &buyer)
                .unwrap()
                .get_id(),
            tx_5_id.get_id()
        );

        tx_3_id
            .get_info(&conn)
            .unwrap()
            .set_transaction_status(TransactionStatus::Paid)
            .update(&conn)
            .unwrap();
        tx_4_id
            .get_info(&conn)
            .unwrap()
            .set_transaction_status(TransactionStatus::Finished)
            .update(&conn)
            .unwrap();
        tx_5_id.refund(&conn).unwrap();

        let expected_stats = TxStats {
            placed_subtotal: 700u32.into(),
            paid_subtotal: ((u32::MAX as usize) * 2).into(),
            finished_subtotal: 700u32.into(),
            refunded_subtotal: 1000u32.into(),
            total: (1400usize + (u32::MAX as usize) * 2).into(),
            placed: 2u32.into(),
            paid: 1u32.into(),
            refunded: 1u32.into(),
            finished: 1u32.into(),
            total_num: 4u32.into(),
        };

        assert_eq!(
            TransactionFinder::stats(&conn, None).unwrap(),
            expected_stats
        );

        assert_eq!(
            TransactionFinder::stats(&conn, Some(&seller)).unwrap(),
            expected_stats
        );
    }
}
