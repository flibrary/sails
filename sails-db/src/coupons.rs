// Coupons should only be readable, writable, and removable by full-right-granted admins due to potential risks on arbitrary code execution.
// DEFAULT coupon should NEVER fail
// All discount amount returned by coupon script should NOT exceed the original total and NEVER be negative

use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    products::ProductInfo,
    schema::coupons,
    script::CouponPackage,
    users::UserInfo,
};
use diesel::{prelude::*, sqlite::Sqlite};
use rhai::{packages::Package, Engine, Scope};
use rocket::FromForm;
use rust_decimal::{prelude::*, Decimal};
use serde::{Deserialize, Serialize};

// A pseudo struct for managing the coupons table.
pub struct Coupons;

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "coupons"]
pub struct Coupon {
    id: String,
    script: String,
}

pub struct CouponContext {
    pub buyer: UserInfo,
    pub seller: UserInfo,
    pub product: ProductInfo,
    pub quantity: i64,
    // Number of time the buyer used this coupon
    pub buyer_used: i64,
    // Number of time this coupon has been used in total
    pub total_used: i64,
}

impl Coupon {
    pub(crate) fn new_without_db(id: impl ToString, script: impl ToString) -> Self {
        Self {
            id: id.to_string(),
            script: script.to_string(),
        }
    }

    pub fn new(
        conn: &SqliteConnection,
        id_p: impl ToString,
        script_p: impl ToString,
    ) -> Result<Self> {
        use crate::schema::coupons::dsl::*;
        let value = Self {
            id: id_p.to_string(),
            script: script_p.to_string(),
        };

        // We don't allow creating reserved coupons
        if (value.get_id() == "_NO_COUPON_APPLIED_") || (value.get_id() == "_BUILTIN_") {
            return Err(SailsDbError::CouponIDReserved);
        }

        diesel::insert_into(coupons)
            .values(value.clone())
            .execute(conn)?;
        Ok(value)
    }

    pub fn create(&self, conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::coupons::dsl::*;
        Ok(diesel::insert_into(coupons).values(self).execute(conn)?)
    }

    pub fn delete(self, conn: &SqliteConnection) -> Result<()> {
        use crate::schema::coupons::dsl::*;
        diesel::delete(coupons.filter(id.eq(&self.id))).execute(conn)?;
        Ok(())
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_script(&self) -> &str {
        &self.script
    }

    pub fn exec(&self, ctx: CouponContext) -> Result<i64> {
        let mut engine = Engine::new();
        engine.register_global_module(CouponPackage::new().as_shared_module());
        // .on_print(|x| log::info!("{}", x))
        // .on_debug(|x, src, pos| log::debug!("{} at {}: {}", x, src.unwrap_or("unkown"), pos));

        // Then turn it into an immutable instance
        let engine = engine;

        let mut scope = Scope::new();

        scope.push_constant("buyer", ctx.buyer);
        scope.push_constant("seller", ctx.seller);
        scope.push_constant("product", ctx.product);
        scope.push_constant("quantity", ctx.quantity);
        scope.push_constant("buyer_used", ctx.buyer_used);
        scope.push_constant("total_used", ctx.total_used);

        let ast = engine.compile(&self.script)?;

        // Try both output type: i64 and decimal
        match engine.eval_ast_with_scope::<i64>(&mut scope, &ast) {
            Ok(r) => Ok(r),
            Err(_) => engine
                .eval_ast_with_scope::<Decimal>(&mut scope, &ast)?
                .to_i64()
                .ok_or(SailsDbError::CouponNotApplicable),
        }
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<Coupon>(conn)?)
    }
}

type BoxedQuery<'a> = coupons::BoxedQuery<'a, Sqlite, coupons::SqlType>;

/// A search query helper (builder)
pub struct CouponFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

impl<'a> CouponFinder<'a> {
    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<Coupon>> {
        Self::new(conn, None).search()
    }

    pub fn new(conn: &'a SqliteConnection, query: Option<BoxedQuery<'a>>) -> Self {
        use crate::schema::coupons::dsl::*;
        if let Some(q) = query {
            Self { conn, query: q }
        } else {
            Self {
                conn,
                query: coupons.into_boxed(),
            }
        }
    }

    pub fn search(self) -> Result<Vec<Coupon>> {
        Ok(self.query.load::<Coupon>(self.conn)?)
    }

    pub fn first(self) -> Result<Coupon> {
        Ok(self.query.first::<Coupon>(self.conn)?)
    }

    pub fn id(mut self, id_provided: &'a str) -> Self {
        use crate::schema::coupons::dsl::*;
        self.query = self.query.filter(id.eq(id_provided));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        categories::{Category, CtgTrait},
        enums::*,
        products::IncompleteProduct,
        test_utils::establish_connection,
        transactions::*,
        users::*,
    };

    #[test]
    fn create_coupon() {
        let conn = establish_connection();
        // our seller
        let seller = UserForm::new("TestUser@example.org", "NFLS", "", None)
            .to_ref()
            .unwrap()
            .create(&conn)
            .unwrap();

        let buyer = UserForm::new("AtypicalBuyer@example.org", "NFLS", "", None)
            .to_ref()
            .unwrap()
            .create(&conn)
            .unwrap();

        buyer
            .get_info(&conn)
            .unwrap()
            .set_user_status(UserStatus::CONTENT_CREATOR)
            .update(&conn)
            .unwrap();

        // The book category
        let econ = Category::create(&conn, "Economics Books", 1)
            .and_then(Category::into_leaf)
            .unwrap();
        let book_id = IncompleteProduct::new(
            &econ,
            "Krugman's Economics 2nd Edition",
            700,
            10,
            "A very great book on the subject of Economics",
            crate::enums::Currency::USD,
        )
        .unwrap()
        .create(&conn, &seller)
        .unwrap();

        // CREATORS 20% OFF
        Coupon::new(
            &conn,
            "DEFAULT",
            r#"if buyer.get_user_status().contains(gen_user_status("CONTENT_CREATOR")) { (product.get_price() * quantity) * 0.2 } else { 0 }"#,
        )
        .unwrap();

        Coupon::new(
            &conn,
            "100OFF",
            r#"if buyer.get_id() == "AtypicalBuyer@example.org" { 100 } else { 0 }"#,
        )
        .unwrap();

        // Verify the book
        book_id
            .get_info(&conn)
            .unwrap()
            .set_product_status(ProductStatus::Verified)
            .update(&conn)
            .unwrap();

        // There should be no transaction entry
        assert_eq!(TransactionFinder::list(&conn).unwrap().len(), 0);

        // Purchase it
        let tx_id = Transactions::buy(
            &conn,
            &book_id,
            &buyer,
            1,
            "258 Huanhu South Road, Dongqian Lake, Ningbo, China",
            "100OFF",
            Payment::Paypal,
        )
        .unwrap();

        assert_eq!(tx_id.get_info(&conn).unwrap().get_price(), 700);
        assert_eq!(tx_id.get_info(&conn).unwrap().get_quantity(), 1);
        assert_eq!(tx_id.get_info(&conn).unwrap().get_coupon(), "100OFF");
        assert_eq!(tx_id.get_info(&conn).unwrap().get_total(), 600u32.into());
        assert_eq!(tx_id.get_info(&conn).unwrap().get_subtotal(), 700u32.into());

        let tx_id = Transactions::buy(
            &conn,
            &book_id,
            &buyer,
            1,
            "258 Huanhu South Road, Dongqian Lake, Ningbo, China",
            "",
            Payment::Paypal,
        )
        .unwrap();

        assert_eq!(tx_id.get_info(&conn).unwrap().get_price(), 700);
        assert_eq!(tx_id.get_info(&conn).unwrap().get_quantity(), 1);
        assert_eq!(tx_id.get_info(&conn).unwrap().get_coupon(), "DEFAULT");
        assert_eq!(tx_id.get_info(&conn).unwrap().get_total(), 560u32.into());
        assert_eq!(tx_id.get_info(&conn).unwrap().get_subtotal(), 700u32.into());
    }
}
