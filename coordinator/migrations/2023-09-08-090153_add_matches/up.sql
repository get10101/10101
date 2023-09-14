-- Your SQL goes here
CREATE TYPE "MatchState_Type" AS ENUM (
    'Pending',
    'Filled',
    'Failed'
);

CREATE TABLE "matches" (
    id UUID PRIMARY KEY,
    match_state "MatchState_Type" NOT NULL,
    order_id UUID REFERENCES orders (trader_order_id) NOT NULL,
    trader_id TEXT NOT NULL,
    -- The order id of the counter party to that match
    match_order_id UUID REFERENCES orders (trader_order_id) NOT NULL,
    -- The trader id of the counter party to that match
    match_trader_id TEXT NOT NULL,
    execution_price REAL NOT NULL,
    quantity REAL NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
