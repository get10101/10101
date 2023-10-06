CREATE TABLE liquidity_request_logs (
    id SERIAL PRIMARY KEY,
    trader_pk TEXT NOT NULL REFERENCES users(pubkey),
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    requested_amount_sats BIGINT NOT NULL,
    liquidity_option integer NOT NULL REFERENCES liquidity_options(id),
    successfully_requested BOOLEAN NOT NULL
);