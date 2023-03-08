-- Your SQL goes here
CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY NOT NULL,
    leverage NUMBER NOT NULL,
    quantity NUMBER NOT NULL,
    contract_symbol TEXT NOT NULL,
    direction TEXT NOT NULL,
    order_type TEXT NOT NULL,
    state TEXT NOT NULL,
    -- might be null if market order
    limit_price NUMBER,
    -- might be null if not yet filled
    execution_price NUMBER
)
