// A module to centralize all interfaces to rhai scripts

use diesel::prelude::*;
use rhai::{
    def_package, export_module, packages::StandardPackage, plugin::*, EvalAltResult,
    ImmutableString,
};
use std::sync::Arc;

trait IntoEvalAltResultError<T> {
    fn into_evalrst_err(self) -> Result<T, Box<EvalAltResult>>;
}

trait IntoEvalAltResultStr<T> {
    fn into_evalrst_str(self) -> Result<T, Box<EvalAltResult>>;
}

impl<T, E: 'static + std::error::Error + Send + Sync> IntoEvalAltResultError<T> for Result<T, E> {
    fn into_evalrst_err(self) -> Result<T, Box<EvalAltResult>> {
        self.map_err(|e| e.to_string()).into_evalrst_str()
    }
}

impl<T, E: AsRef<str> + Send + Sync> IntoEvalAltResultStr<T> for Result<T, E> {
    fn into_evalrst_str(self) -> Result<T, Box<EvalAltResult>> {
        self.map_err(|e| -> Box<EvalAltResult> { e.into() })
    }
}

#[rustfmt::skip]
def_package! {
    // Package used by coupon state machines
    pub CouponPackage(module) {
	StandardPackage::init(module);

	combine_with_exported_module!(module, "utils", self::rhai_mod);
    }
}

#[export_module]
pub mod rhai_mod {
    pub mod users {
        use crate::{enums::UserStatus, users::*};

        pub mod user_id {
            #[rhai_fn(pure, return_raw)]
            pub fn get_info(
                user_id: &mut UserId,
                conn: Arc<SqliteConnection>,
            ) -> Result<UserInfo, Box<EvalAltResult>> {
                user_id.get_info(&conn).into_evalrst_err()
            }

            #[rhai_fn(pure)]
            pub fn get_id(user_id: &mut UserId) -> ImmutableString {
                user_id.get_id().into()
            }
        }

        pub mod user_info {
            use crate::users::UserInfo;

            #[rhai_fn(pure)]
            pub fn get_id(info: &mut UserInfo) -> ImmutableString {
                info.get_id().into()
            }

            #[rhai_fn(pure)]
            pub fn get_school(info: &mut UserInfo) -> ImmutableString {
                info.get_school().into()
            }

            #[rhai_fn(pure)]
            pub fn get_name(info: &mut UserInfo) -> ImmutableString {
                info.get_name().into()
            }

            #[rhai_fn(pure)]
            pub fn get_user_status(info: &mut UserInfo) -> UserStatus {
                info.get_user_status()
            }
        }

        pub mod user_status {
            pub fn gen_user_status(name: ImmutableString) -> UserStatus {
                match name.as_str() {
                    "DISABLED" => UserStatus::DISABLED,
                    "NORMAL" => UserStatus::NORMAL,
                    "CONTENT_CREATOR" => UserStatus::CONTENT_CREATOR,
                    "CUSTOMER_SERVICE" => UserStatus::CUSTOMER_SERVICE,
                    "STORE_KEEPER" => UserStatus::STORE_KEEPER,
                    "ADMIN" => UserStatus::ADMIN,
                    _ => UserStatus::empty(),
                }
            }

            #[rhai_fn(pure)]
            pub fn contains(status: &mut UserStatus, other: UserStatus) -> bool {
                status.contains(other)
            }

            #[rhai_fn(pure)]
            pub fn is_all(status: &mut UserStatus) -> bool {
                status.is_all()
            }

            #[rhai_fn(pure)]
            pub fn is_empty(status: &mut UserStatus) -> bool {
                status.is_empty()
            }
        }
    }

    pub mod products {
        use crate::products::*;

        pub mod product_info {
            #[rhai_fn(pure)]
            pub fn get_seller_id(info: &mut ProductInfo) -> ImmutableString {
                info.get_seller_id().into()
            }

            #[rhai_fn(pure)]
            pub fn get_description(info: &mut ProductInfo) -> ImmutableString {
                info.get_description().into()
            }

            #[rhai_fn(pure)]
            pub fn get_category_id(info: &mut ProductInfo) -> ImmutableString {
                info.get_category_id().into()
            }

            #[rhai_fn(pure)]
            pub fn get_prodname(info: &mut ProductInfo) -> ImmutableString {
                info.get_prodname().into()
            }

            #[rhai_fn(pure)]
            pub fn get_price(info: &mut ProductInfo) -> i64 {
                info.get_price() as i64
            }

            #[rhai_fn(pure)]
            pub fn get_quantity(info: &mut ProductInfo) -> i64 {
                info.get_price() as i64
            }

            #[rhai_fn(pure)]
            pub fn get_currency(info: &mut ProductInfo) -> ImmutableString {
                format!("{:?}", info.get_currency()).into()
            }
        }
    }

    pub mod transactions {}
}
