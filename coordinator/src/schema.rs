// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "BonusStatus_Type"))]
    pub struct BonusStatusType;

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
    #[diesel(postgres_type(name = "Dlc_Channel_State_Type"))]
    pub struct DlcChannelStateType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Htlc_Status_Type"))]
    pub struct HtlcStatusType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "MatchState_Type"))]
    pub struct MatchStateType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Message_Type_Type"))]
    pub struct MessageTypeType;

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
    #[diesel(postgres_type(name = "Poll_Type_Type"))]
    pub struct PollTypeType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "PositionState_Type"))]
    pub struct PositionStateType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Protocol_State_Type"))]
    pub struct ProtocolStateType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "Protocol_Type_Type"))]
    pub struct ProtocolTypeType;
}

diesel::table! {
    answers (id) {
        id -> Int4,
        choice_id -> Int4,
        trader_pubkey -> Text,
        value -> Text,
        creation_timestamp -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::BonusStatusType;

    bonus_status (id) {
        id -> Int4,
        trader_pubkey -> Text,
        tier_level -> Int4,
        fee_rebate -> Float4,
        bonus_type -> BonusStatusType,
        activation_timestamp -> Timestamptz,
        deactivation_timestamp -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::BonusStatusType;

    bonus_tiers (id) {
        id -> Int4,
        tier_level -> Int4,
        min_users_to_refer -> Int4,
        fee_rebate -> Float4,
        bonus_tier_type -> BonusStatusType,
        active -> Bool,
    }
}

diesel::table! {
    channel_opening_params (order_id) {
        order_id -> Text,
        coordinator_reserve -> Int8,
        trader_reserve -> Int8,
        created_at -> Int8,
    }
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
        liquidity_option_id -> Nullable<Int4>,
        fee_sats -> Nullable<Int8>,
    }
}

diesel::table! {
    choices (id) {
        id -> Int4,
        poll_id -> Int4,
        value -> Text,
        editable -> Bool,
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
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::DlcChannelStateType;

    dlc_channels (id) {
        id -> Int4,
        open_protocol_id -> Uuid,
        channel_id -> Text,
        trader_pubkey -> Text,
        channel_state -> DlcChannelStateType,
        trader_reserve_sats -> Int8,
        coordinator_reserve_sats -> Int8,
        funding_txid -> Nullable<Text>,
        close_txid -> Nullable<Text>,
        settle_txid -> Nullable<Text>,
        buffer_txid -> Nullable<Text>,
        claim_txid -> Nullable<Text>,
        punish_txid -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        coordinator_funding_sats -> Int8,
        trader_funding_sats -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::MessageTypeType;

    dlc_messages (message_hash) {
        message_hash -> Text,
        inbound -> Bool,
        peer_id -> Text,
        message_type -> MessageTypeType,
        timestamp -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ProtocolStateType;
    use super::sql_types::ProtocolTypeType;

    dlc_protocols (id) {
        id -> Int4,
        protocol_id -> Uuid,
        previous_protocol_id -> Nullable<Uuid>,
        channel_id -> Text,
        contract_id -> Text,
        protocol_state -> ProtocolStateType,
        trader_pubkey -> Text,
        timestamp -> Timestamptz,
        protocol_type -> ProtocolTypeType,
    }
}

diesel::table! {
    last_outbound_dlc_messages (peer_id) {
        peer_id -> Text,
        message_hash -> Text,
        message -> Text,
        timestamp -> Timestamptz,
    }
}

diesel::table! {
    legacy_collaborative_reverts (id) {
        id -> Int4,
        channel_id -> Text,
        trader_pubkey -> Text,
        price -> Float4,
        coordinator_address -> Text,
        coordinator_amount_sats -> Int8,
        trader_amount_sats -> Int8,
        funding_txid -> Text,
        funding_vout -> Int4,
        timestamp -> Timestamptz,
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
        matching_fee_sats -> Int8,
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
    use super::sql_types::PollTypeType;

    polls (id) {
        id -> Int4,
        poll_type -> PollTypeType,
        question -> Text,
        active -> Bool,
        creation_timestamp -> Timestamptz,
        whitelisted -> Bool,
    }
}

diesel::table! {
    polls_whitelist (id) {
        id -> Int4,
        poll_id -> Int4,
        trader_pubkey -> Text,
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
        trader_direction -> DirectionType,
        average_entry_price -> Float4,
        trader_liquidation_price -> Float4,
        position_state -> PositionStateType,
        coordinator_margin -> Int8,
        creation_timestamp -> Timestamptz,
        expiry_timestamp -> Timestamptz,
        update_timestamp -> Timestamptz,
        trader_pubkey -> Text,
        temporary_contract_id -> Nullable<Text>,
        trader_realized_pnl_sat -> Nullable<Int8>,
        trader_unrealized_pnl_sat -> Nullable<Int8>,
        closing_price -> Nullable<Float4>,
        coordinator_leverage -> Float4,
        trader_margin -> Int8,
        stable -> Bool,
        coordinator_liquidation_price -> Float4,
        order_matching_fees -> Int8,
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
    use super::sql_types::DirectionType;

    trade_params (id) {
        id -> Int4,
        protocol_id -> Uuid,
        trader_pubkey -> Text,
        quantity -> Float4,
        leverage -> Float4,
        average_price -> Float4,
        direction -> DirectionType,
        matching_fee -> Int8,
        trader_pnl_sat -> Nullable<Int8>,
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
        direction -> DirectionType,
        average_price -> Float4,
        timestamp -> Timestamptz,
        order_matching_fee_sat -> Int8,
        trader_realized_pnl_sat -> Nullable<Int8>,
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
        contact -> Text,
        timestamp -> Timestamptz,
        fcm_token -> Text,
        last_login -> Timestamptz,
        nickname -> Nullable<Text>,
        version -> Nullable<Text>,
        referral_code -> Text,
        used_referral_code -> Nullable<Text>,
    }
}

diesel::joinable!(answers -> choices (choice_id));
diesel::joinable!(choices -> polls (poll_id));
diesel::joinable!(last_outbound_dlc_messages -> dlc_messages (message_hash));
diesel::joinable!(liquidity_request_logs -> liquidity_options (liquidity_option));
diesel::joinable!(polls_whitelist -> polls (poll_id));
diesel::joinable!(trades -> positions (position_id));

diesel::allow_tables_to_appear_in_same_query!(
    answers,
    bonus_status,
    bonus_tiers,
    channel_opening_params,
    channels,
    choices,
    collaborative_reverts,
    dlc_channels,
    dlc_messages,
    dlc_protocols,
    last_outbound_dlc_messages,
    legacy_collaborative_reverts,
    liquidity_options,
    liquidity_request_logs,
    matches,
    orders,
    payments,
    polls,
    polls_whitelist,
    positions,
    routing_fees,
    spendable_outputs,
    trade_params,
    trades,
    transactions,
    users,
);
