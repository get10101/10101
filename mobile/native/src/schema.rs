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
        creation_timestamp -> BigInt,
        limit_price -> Nullable<Double>,
        execution_price -> Nullable<Double>,
        failure_reason -> Nullable<Text>,
    }
}

diesel::table! {
    positions (contract_symbol) {
        contract_symbol -> Text,
        leverage -> Double,
        quantity -> Double,
        direction -> Text,
        average_entry_price -> Double,
        liquidation_price -> Double,
        state -> Text,
        collateral -> BigInt,
        creation_timestamp -> BigInt,
    }
}

diesel::allow_tables_to_appear_in_same_query!(last_login, orders, positions,);
