-- Your SQL goes here
CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY NOT NULL,
    leverage FLOAT NOT NULL,
    quantity FLOAT NOT NULL,
    contract_symbol TEXT NOT NULL,
    direction TEXT NOT NULL,
    order_type TEXT NOT NULL,
    state TEXT NOT NULL,
    creation_timestamp BIGINT NOT NULL,
    -- might be null if market order
    limit_price FLOAT,
    -- might be null if not yet matched
    execution_price FLOAT,
    -- might be null ig there was no failure
    failure_reason TEXT
)
