CREATE TABLE "legacy_collaborative_reverts"
(
    id                      SERIAL PRIMARY KEY       NOT NULL,
    channel_id              TEXT                     NOT NULL REFERENCES channels (channel_id),
    trader_pubkey           TEXT                     NOT NULL REFERENCES users (pubkey),
    price                   REAL                     NOT NULL,
    coordinator_address     TEXT                     NOT NULL,
    coordinator_amount_sats BIGINT                   NOT NULL,
    trader_amount_sats      BIGINT                   NOT NULL,
    funding_txid            TEXT                     NOT NULL,
    funding_vout            INT                      NOT NULL,
    timestamp               timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
