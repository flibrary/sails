table! {
    categories (id) {
        id -> Text,
        name -> Text,
        parent_id -> Nullable<Text>,
        is_leaf -> Bool,
    }
}

table! {
    messages (id) {
        id -> Text,
        send -> Text,
        recv -> Text,
        body -> Text,
        time_sent -> Timestamp,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::enums::*;

    products (id) {
        id -> Text,
        shortid -> Text,
        seller_id -> Text,
        operator_id -> Text,
        category -> Text,
        prodname -> Text,
        price -> BigInt,
        description -> Text,
        product_status -> ProductStatusMapping,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::enums::*;

    transactions (id) {
        id -> Text,
        shortid -> Text,
        seller -> Text,
        product -> Text,
        buyer -> Text,
        time_sent -> Timestamp,
        transaction_status -> TransactionStatusMapping,
    }
}

table! {
    users (id) {
        id -> Text,
        name -> Text,
        school -> Text,
        hashed_passwd -> Text,
        validated -> Bool,
        description -> Nullable<Text>,
        user_status -> BigInt,
    }
}

joinable!(products -> categories (category));
joinable!(transactions -> products (product));

allow_tables_to_appear_in_same_query!(categories, messages, products, transactions, users,);
