ALTER TABLE trades
    RENAME COLUMN trader_leverage TO leverage;

ALTER TABLE positions
    RENAME COLUMN trader_leverage TO leverage;
