CREATE TABLE IF NOT EXISTS trades (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    order_id TEXT NOT NULL,
    contract_symbol TEXT NOT NULL,
    contracts FLOAT NOT NULL,
    direction TEXT NOT NULL,
    trade_cost_sat BIGINT NOT NULL,
    fee_sat BIGINT NOT NULL,
    pnl_sat BIGINT,
    price FLOAT NOT NULL,
    timestamp BIGINT NOT NULL
)
