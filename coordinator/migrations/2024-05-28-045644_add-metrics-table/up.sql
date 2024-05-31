create table if not exists metrics
(
    id                    SERIAL PRIMARY KEY       NOT NULL,
    created_at            timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    on_chain_balance_sats BIGINT                   NOT NULL
);
