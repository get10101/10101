CREATE TABLE funding_fee_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    contract_symbol TEXT NOT NULL,
    contracts FLOAT NOT NULL,
    direction TEXT NOT NULL,
    price FLOAT NOT NULL,
    fee BIGINT NOT NULL,
    due_date BIGINT NOT NULL,
    paid_date BIGINT
);

CREATE UNIQUE INDEX idx_unique_due_date_contract_symbol ON funding_fee_events (due_date, contract_symbol);
