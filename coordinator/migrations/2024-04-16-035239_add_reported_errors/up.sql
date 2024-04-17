CREATE TABLE reported_errors (
    id SERIAL PRIMARY KEY NOT NULL,
    trader_pubkey TEXT NOT NULL,
    error TEXT NOT NULL
);
