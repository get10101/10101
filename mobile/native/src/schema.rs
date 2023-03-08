// @generated automatically by Diesel CLI.

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
        state -> Text,
        limit_price -> Nullable<Double>,
        execution_price -> Nullable<Double>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(last_login, orders,);
