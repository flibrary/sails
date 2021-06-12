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
        school -> Text,
        phone -> Text,
        hashed_passwd -> Text,
    }
}

joinable!(products -> categories (category));
joinable!(products -> users (seller_id));

allow_tables_to_appear_in_same_query!(
    categories,
    messages,
    products,
    users,
);
