// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "ChannelState_Type"))]
    pub struct ChannelStateType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "ContractSymbol_Type"))]
    pub struct ContractSymbolType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Direction_Type"))]
    pub struct DirectionType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Htlc_Status_Type"))]
    pub struct HtlcStatusType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "MatchState_Type"))]
    pub struct MatchStateType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "OrderReason_Type"))]
    pub struct OrderReasonType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "OrderState_Type"))]
    pub struct OrderStateType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "OrderType_Type"))]
    pub struct OrderTypeType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Payment_Flow_Type"))]
    pub struct PaymentFlowType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "PositionState_Type"))]
    pub struct PositionStateType;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ChannelStateType;

    channels (user_channel_id) {
        user_channel_id -> Text,
        channel_id -> Nullable<Text>,
        inbound_sats -> Int8,
        outbound_sats -> Int8,
        funding_txid -> Nullable<Text>,
        channel_state -> ChannelStateType,
        counterparty_pubkey -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        open_channel_fee_payment_hash -> Nullable<Text>,
        liquidity_option_id -> Nullable<Int4>,
    }
}

diesel::table! {
    collaborative_reverts (id) {
        id -> Int4,
        channel_id -> Text,
        trader_pubkey -> Text,
        price -> Float4,
        coordinator_address -> Text,
        coordinator_amount_sats -> Int8,
        trader_amount_sats -> Int8,
        timestamp -> Timestamptz,
        funding_txid -> Text,
        funding_vout -> Int4,
    }
}

diesel::table! {
    liquidity_options (id) {
        id -> Int4,
        rank -> Int2,
        title -> Text,
        trade_up_to_sats -> Int8,
        min_deposit_sats -> Int8,
        max_deposit_sats -> Int8,
        min_fee_sats -> Nullable<Int8>,
        fee_percentage -> Float8,
        coordinator_leverage -> Float4,
        active -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    liquidity_request_logs (id) {
        id -> Int4,
        trader_pk -> Text,
        timestamp -> Timestamptz,
        requested_amount_sats -> Int8,
        liquidity_option -> Int4,
        successfully_requested -> Bool,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::MatchStateType;

    matches (id) {
        id -> Uuid,
        match_state -> MatchStateType,
        order_id -> Uuid,
        trader_id -> Text,
        match_order_id -> Uuid,
        match_trader_id -> Text,
        execution_price -> Float4,
        quantity -> Float4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::DirectionType;
    use super::sql_types::OrderTypeType;
    use super::sql_types::OrderStateType;
    use super::sql_types::ContractSymbolType;
    use super::sql_types::OrderReasonType;

    orders (id) {
        id -> Int4,
        trader_order_id -> Uuid,
        price -> Float4,
        trader_id -> Text,
        direction -> DirectionType,
        quantity -> Float4,
        timestamp -> Timestamptz,
        order_type -> OrderTypeType,
        expiry -> Timestamptz,
        order_state -> OrderStateType,
        contract_symbol -> ContractSymbolType,
        leverage -> Float4,
        order_reason -> OrderReasonType,
        stable -> Bool,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::HtlcStatusType;
    use super::sql_types::PaymentFlowType;

    payments (id) {
        id -> Int4,
        payment_hash -> Text,
        preimage -> Nullable<Text>,
        secret -> Nullable<Text>,
        htlc_status -> HtlcStatusType,
        amount_msat -> Nullable<Int8>,
        flow -> PaymentFlowType,
        payment_timestamp -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        description -> Text,
        invoice -> Nullable<Text>,
        fee_msat -> Nullable<Int8>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ContractSymbolType;
    use super::sql_types::DirectionType;
    use super::sql_types::PositionStateType;

    positions (id) {
        id -> Int4,
        contract_symbol -> ContractSymbolType,
        trader_leverage -> Float4,
        quantity -> Float4,
        direction -> DirectionType,
        average_entry_price -> Float4,
        liquidation_price -> Float4,
        position_state -> PositionStateType,
        coordinator_margin -> Int8,
        creation_timestamp -> Timestamptz,
        expiry_timestamp -> Timestamptz,
        update_timestamp -> Timestamptz,
        trader_pubkey -> Text,
        temporary_contract_id -> Nullable<Text>,
        realized_pnl_sat -> Nullable<Int8>,
        unrealized_pnl_sat -> Nullable<Int8>,
        closing_price -> Nullable<Float4>,
        coordinator_leverage -> Float4,
        trader_margin -> Int8,
        stable -> Bool,
    }
}

diesel::table! {
    routing_fees (id) {
        id -> Int4,
        amount_msats -> Int8,
        prev_channel_id -> Nullable<Text>,
        next_channel_id -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    spendable_outputs (id) {
        id -> Int4,
        txid -> Text,
        vout -> Int4,
        descriptor -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ContractSymbolType;
    use super::sql_types::DirectionType;

    trades (id) {
        id -> Int4,
        position_id -> Int4,
        contract_symbol -> ContractSymbolType,
        trader_pubkey -> Text,
        quantity -> Float4,
        trader_leverage -> Float4,
        collateral -> Int8,
        direction -> DirectionType,
        average_price -> Float4,
        timestamp -> Timestamptz,
        fee_payment_hash -> Text,
    }
}

diesel::table! {
    transactions (txid) {
        txid -> Text,
        fee -> Int8,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        raw -> Text,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        pubkey -> Text,
        email -> Text,
        nostr -> Text,
        timestamp -> Timestamptz,
        fcm_token -> Text,
        last_login -> Timestamptz,
    }
}

diesel::joinable!(liquidity_request_logs -> liquidity_options (liquidity_option));
diesel::joinable!(trades -> positions (position_id));

diesel::allow_tables_to_appear_in_same_query!(
    channels,
    collaborative_reverts,
    liquidity_options,
    liquidity_request_logs,
    matches,
    orders,
    payments,
    positions,
    routing_fees,
    spendable_outputs,
    trades,
    transactions,
    users,
);
