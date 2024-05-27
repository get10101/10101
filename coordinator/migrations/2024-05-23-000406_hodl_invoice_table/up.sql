create table if not exists hodl_invoices
(
    id               SERIAL PRIMARY KEY       NOT NULL,
    trader_pubkey    TEXT                     NOT NULL REFERENCES users (pubkey),
    r_hash           TEXT                     NOT NULL,
    amount_sats      BIGINT                   NOT NULL,
    pre_image        TEXT,
    created_at       timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at        timestamp WITH TIME ZONE
)
