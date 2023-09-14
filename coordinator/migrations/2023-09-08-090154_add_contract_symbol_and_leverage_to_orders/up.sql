-- Your SQL goes here
ALTER TABLE "orders"
    ADD COLUMN "contract_symbol" "ContractSymbol_Type" NOT NULL DEFAULT 'BtcUsd',
    ADD COLUMN "leverage" REAL NOT NULL DEFAULT 1.0;
