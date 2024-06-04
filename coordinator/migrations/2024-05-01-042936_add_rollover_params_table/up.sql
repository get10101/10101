CREATE TABLE rollover_params
(
    id SERIAL PRIMARY KEY NOT NULL,
    protocol_id UUID NOT NULL REFERENCES dlc_protocols (protocol_id),
    trader_pubkey TEXT NOT NULL,
    margin_coordinator_sat BIGINT NOT NULL,
    margin_trader_sat BIGINT NOT NULL,
    leverage_coordinator REAL NOT NULL,
    leverage_trader REAL NOT NULL,
    liquidation_price_coordinator REAL NOT NULL,
    liquidation_price_trader REAL NOT NULL,
    expiry_timestamp TIMESTAMP WITH TIME ZONE NOT NULL
);
