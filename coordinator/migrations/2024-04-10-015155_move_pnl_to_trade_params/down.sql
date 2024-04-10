ALTER TABLE trades
      ADD COLUMN is_complete BOOLEAN NOT NULL DEFAULT true;

ALTER TABLE trade_params
      DROP COLUMN IF EXISTS trader_pnl_sat;
