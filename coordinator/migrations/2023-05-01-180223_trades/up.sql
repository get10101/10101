-- Your SQL goes here
ALTER TABLE
    users
ADD
    COLUMN timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP;
CREATE TABLE "trades" (
    id SERIAL PRIMARY KEY NOT NULL,
    position_id integer REFERENCES positions (id),
    contract_symbol "ContractSymbol_Type" NOT NULL,
    trader_pubkey TEXT NOT NULL,
    quantity REAL NOT NULL,
    leverage REAL NOT NULL,
    our_collateral BIGINT NOT NULL,
    direction "Direction_Type" NOT NULL,
    average_price REAL NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
