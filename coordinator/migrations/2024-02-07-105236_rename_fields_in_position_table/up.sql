ALTER TABLE positions
    RENAME COLUMN direction TO trader_direction;

ALTER TABLE positions
    RENAME COLUMN liquidation_price TO trader_liquidation_price;

ALTER TABLE positions
    RENAME COLUMN realized_pnl_sat TO trader_realized_pnl_sat;

ALTER TABLE positions
    RENAME COLUMN unrealized_pnl_sat TO trader_unrealized_pnl_sat;
