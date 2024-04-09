// @generated automatically by Diesel CLI.

diesel::table! {
    answered_polls (id) {
        id -> Integer,
        poll_id -> Integer,
        timestamp -> BigInt,
    }
}

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
        fee_sats -> Nullable<BigInt>,
        open_channel_payment_hash -> Nullable<Text>,
    }
}

diesel::table! {
    dlc_messages (message_hash) {
        message_hash -> Text,
        inbound -> Bool,
        peer_id -> Text,
        message_type -> Text,
        timestamp -> BigInt,
    }
}

diesel::table! {
    ignored_polls (id) {
        id -> Integer,
        poll_id -> Integer,
        timestamp -> BigInt,
    }
}

diesel::table! {
    last_outbound_dlc_messages (peer_id) {
        peer_id -> Text,
        message_hash -> Text,
        message -> Text,
        timestamp -> BigInt,
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
        matching_fee_sats -> Nullable<BigInt>,
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
        funding_txid -> Nullable<Text>,
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
    trades (id) {
        id -> Integer,
        order_id -> Text,
        contract_symbol -> Text,
        contracts -> Float,
        direction -> Text,
        trade_cost_sat -> BigInt,
        fee_sat -> BigInt,
        pnl_sat -> Nullable<BigInt>,
        price -> Float,
        timestamp -> BigInt,
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

diesel::joinable!(last_outbound_dlc_messages -> dlc_messages (message_hash));

diesel::allow_tables_to_appear_in_same_query!(
    answered_polls,
    channels,
    dlc_messages,
    ignored_polls,
    last_outbound_dlc_messages,
    orders,
    payments,
    positions,
    spendable_outputs,
    trades,
    transactions,
);
