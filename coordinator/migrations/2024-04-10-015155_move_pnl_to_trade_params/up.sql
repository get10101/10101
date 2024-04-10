ALTER TABLE trades
      DROP COLUMN IF EXISTS "is_complete";

ALTER TABLE trade_params
      ADD COLUMN trader_pnl_sat BIGINT;
