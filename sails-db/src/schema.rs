table! {
    categories (id) {
        id -> Text,
        name -> Text,
        priority -> BigInt,
        parent_id -> Nullable<Text>,
        is_leaf -> Bool,
    }
}

table! {
    digiconmappings (id) {
        id -> Text,
        digicon -> Text,
        product -> Text,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::enums::*;

    digicons (id) {
        id -> Text,
        creator_id -> Text,
        name -> Text,
        storage_type -> StorageTypeMapping,
        storage_detail -> Nullable<Text>,
        time_created -> Timestamp,
        time_modified -> Timestamp,
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
        category -> Text,
        prodname -> Text,
        price -> BigInt,
        quantity -> BigInt,
        description -> Text,
        product_status -> ProductStatusMapping,
        currency -> CurrencyMapping,
    }
}

table! {
    tagmappings (id) {
        id -> Text,
        tag -> Text,
        product -> Text,
    }
}

table! {
    tags (id) {
        id -> Text,
        name -> Text,
        html -> Nullable<Text>,
        description -> Nullable<Text>,
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
        price -> BigInt,
        quantity -> BigInt,
        address -> Text,
        time_sent -> Timestamp,
        transaction_status -> TransactionStatusMapping,
        payment -> PaymentMapping,
        currency -> CurrencyMapping,
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

joinable!(digiconmappings -> digicons (digicon));
joinable!(digiconmappings -> products (product));
joinable!(digicons -> users (creator_id));
joinable!(products -> categories (category));
joinable!(products -> users (seller_id));
joinable!(tagmappings -> products (product));
joinable!(tagmappings -> tags (tag));
joinable!(transactions -> products (product));

allow_tables_to_appear_in_same_query!(
    categories,
    digiconmappings,
    digicons,
    messages,
    products,
    tagmappings,
    tags,
    transactions,
    users,
);
