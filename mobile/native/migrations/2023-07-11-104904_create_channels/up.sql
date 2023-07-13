-- Your SQL goes here
CREATE TABLE "channels" (
    user_channel_id TEXT PRIMARY KEY NOT NULL,
    channel_id TEXT UNIQUE,
    capacity BIGINT NOT NULL,
    funding_txid TEXT,
    channel_state TEXT NOT NULL,
    trader_pubkey TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    -- this value is stored here for simplicity of creating a sql query. However, it is not the source of truth and can
    -- be recreated from the various transactions attached to the channel.
    costs BIGINT NOT NULL DEFAULT 0
);
