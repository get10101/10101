// @generated automatically by Diesel CLI.

diesel::table! {
    channels (user_channel_id) {
        user_channel_id -> Text,
        channel_id -> Nullable<Text>,
        inbound -> BigInt,
        outbound -> BigInt,
        funding_txid -> Nullable<Text>,
        channel_state -> Text,
        counterparty_pubkey -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
        liquidity_option_id -> Nullable<Integer>,
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
        leverage -> Float,
        quantity -> Float,
        contract_symbol -> Text,
        direction -> Text,
        order_type -> Text,
        state -> Text,
        creation_timestamp -> BigInt,
        limit_price -> Nullable<Float>,
        execution_price -> Nullable<Float>,
        failure_reason -> Nullable<Text>,
        order_expiry_timestamp -> BigInt,
        reason -> Text,
        stable -> Bool,
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
        description -> Text,
        invoice -> Nullable<Text>,
        fee_msat -> Nullable<BigInt>,
    }
}

diesel::table! {
    positions (contract_symbol) {
        contract_symbol -> Text,
        leverage -> Float,
        quantity -> Float,
        direction -> Text,
        average_entry_price -> Float,
        liquidation_price -> Float,
        state -> Text,
        collateral -> BigInt,
        creation_timestamp -> BigInt,
        expiry_timestamp -> BigInt,
        updated_timestamp -> BigInt,
        stable -> Bool,
    }
}

diesel::table! {
    spendable_outputs (id) {
        id -> Integer,
        outpoint -> Text,
        descriptor -> Text,
    }
}

diesel::table! {
    transactions (txid) {
        txid -> Text,
        fee -> BigInt,
        created_at -> BigInt,
        updated_at -> BigInt,
        raw -> Text,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    channels,
    last_login,
    orders,
    payments,
    positions,
    spendable_outputs,
    transactions,
);
