ALTER TABLE trades
    RENAME COLUMN leverage TO trader_leverage;
ALTER TABLE positions
    RENAME COLUMN leverage TO trader_leverage;
