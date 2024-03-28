ALTER TABLE "trades"
    ADD COLUMN "trader_realized_pnl_sat" BIGINT;

UPDATE TRADES SET trader_realized_pnl_sat = (SELECT trader_realized_pnl_sat from POSITIONS where POSITIONS.ID = TRADES.POSITION_ID AND POSITIONS.TRADER_DIRECTION != TRADES.DIRECTION);
