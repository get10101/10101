-- Your SQL goes here
-- All transactions broadcasted by us
CREATE TABLE "transactions" (
    txid TEXT PRIMARY KEY NOT NULL,
    -- the fee is stored here for simplicity of creating a sql query. However, it is not the source of truth and can
    -- be recreated from looking up the transaction on the blockchain.
    fee BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE TABLE "channels" (
    user_channel_id TEXT PRIMARY KEY NOT NULL,
    channel_id TEXT UNIQUE,
    inbound BIGINT NOT NULL,
    outbound BIGINT NOT NULL,
    funding_txid TEXT,
    channel_state TEXT NOT NULL,
    counterparty_pubkey TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
