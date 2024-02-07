ALTER TABLE positions
    RENAME COLUMN trader_direction TO direction;

ALTER TABLE positions
    RENAME COLUMN trader_liquidation_price TO liquidation_price;

ALTER TABLE positions
    RENAME COLUMN trader_realized_pnl_sat TO realized_pnl_sat;

ALTER TABLE positions
    RENAME COLUMN trader_unrealized_pnl_sat TO unrealized_pnl_sat;
