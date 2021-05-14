table! {
    products (id) {
        id -> Text,
        seller_id -> Text,
        prodname -> Text,
        price -> Integer,
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

joinable!(products -> users (seller_id));

allow_tables_to_appear_in_same_query!(
    products,
    users,
);
