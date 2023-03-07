// @generated automatically by Diesel CLI.

diesel::table! {
    directions (direction) {
        direction -> Text,
    }
}

diesel::table! {
    last_login (id) {
        id -> Nullable<Integer>,
        date -> Text,
    }
}

diesel::table! {
    orders (id) {
        id -> Text,
        leverage -> Double,
        quantity -> Double,
        contract_symbol -> Text,
        direction -> Text,
        order_type -> Text,
        status -> Text,
        limit_price -> Nullable<Double>,
        execution_price -> Nullable<Double>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(directions, last_login, orders,);
