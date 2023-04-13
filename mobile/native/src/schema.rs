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
    payments (id) {
        id -> Integer,
        payment_hash -> Text,
        preimage -> Nullable<Text>,
        secret -> Nullable<Text>,
        htlc_status -> Text,
        amount_msat -> Nullable<BigInt>,
        flow -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
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

diesel::allow_tables_to_appear_in_same_query!(last_login, orders, payments, positions,);
