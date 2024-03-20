CREATE TYPE "Dlc_Channel_State_Type" AS ENUM ('Pending', 'Open', 'Closing', 'Closed', 'Failed', 'Cancelled');

CREATE TABLE "dlc_channels"
(
    id                          SERIAL PRIMARY KEY                          NOT NULL,
    -- points to the dlc protocol that opened the channel
    open_protocol_id            UUID                                        NOT NULL,
    channel_id                  TEXT                                        NOT NULL,
    trader_pubkey               TEXT REFERENCES users (pubkey)              NOT NULL,
    channel_state               "Dlc_Channel_State_Type"                    NOT NULL,
    trader_reserve_sats         BIGINT                                      NOT NULL,
    coordinator_reserve_sats    BIGINT                                      NOT NULL,
    funding_txid                TEXT,
    close_txid                  TEXT,
    settle_txid                 TEXT,
    buffer_txid                 TEXT,
    claim_txid                  TEXT,
    punish_txid                 TEXT,
    created_at timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
