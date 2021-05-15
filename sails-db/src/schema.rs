table! {
    categories (id) {
        id -> Text,
        ctgname -> Text,
        parent_id -> Nullable<Text>,
        is_leaf -> Bool,
    }
}

table! {
    products (id) {
        id -> Text,
        seller_id -> Text,
        category -> Text,
        prodname -> Text,
        price -> BigInt,
        description -> Text,
    }
}

table! {
    users (id) {
        id -> Text,
        email -> Nullable<Text>,
        school -> Text,
        phone -> Text,
        hashed_passwd -> Text,
    }
}

joinable!(products -> categories (category));
joinable!(products -> users (seller_id));

allow_tables_to_appear_in_same_query!(
    categories,
    products,
    users,
);
