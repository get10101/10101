-- Your SQL goes here
CREATE TYPE "ChannelState_Type" AS ENUM (
    'Pending',
    'Open',
    'Closed',
    'ForceClosedRemote',
    'ForceClosedLocal'
);
-- All transactions broadcasted by us
CREATE TABLE "transactions" (
    txid TEXT PRIMARY KEY,
    -- the fee is stored here for simplicity of creating a sql query. However, it is not the source of truth and can
    -- be recreated from looking up the transaction on the blockchain.
    fee BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE "channels" (
    user_channel_id TEXT PRIMARY KEY,
    channel_id TEXT UNIQUE,
    inbound BIGINT NOT NULL,
    outbound BIGINT NOT NULL,
    funding_txid TEXT,
    channel_state "ChannelState_Type" NOT NULL,
    counterparty_pubkey TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
