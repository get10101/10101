-- Your SQL goes here
-- TODO: Review this model, using the contract symbol as PK is not wrong, but it will depend on how we want to treat positions after closing them
CREATE TABLE IF NOT EXISTS positions (
    contract_symbol TEXT PRIMARY KEY NOT NULL,
    leverage NUMBER NOT NULL,
    quantity NUMBER NOT NULL,
    direction TEXT NOT NULL,
    average_entry_price NUMBER NOT NULL,
    liquidation_price NUMBER NOT NULL,
    state TEXT NOT NULL,
    collateral BIGINT NOT NULL,
    creation_timestamp BIGINT NOT NULL
)
