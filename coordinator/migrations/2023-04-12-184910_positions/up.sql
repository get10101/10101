-- Your SQL goes here
CREATE TYPE "ContractSymbol_Type" AS ENUM ('BtcUsd');
CREATE TYPE "PositionState_Type" AS ENUM ('Open', 'Closing');
CREATE TABLE "positions" (
    id SERIAL PRIMARY KEY NOT NULL,
    contract_symbol "ContractSymbol_Type" NOT NULL,
    leverage REAL NOT NULL,
    quantity REAL NOT NULL,
    direction "Direction_Type" NOT NULL,
    average_entry_price REAL NOT NULL,
    liquidation_price REAL NOT NULL,
    position_state "PositionState_Type" NOT NULL,
    collateral BIGINT NOT NULL,
    creation_timestamp timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expiry_timestamp timestamp WITH TIME ZONE NOT NULL,
    update_timestamp timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    trader_pubkey TEXT NOT NULL
);
