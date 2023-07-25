-- This file should undo anything in `up.sql`
ALTER TABLE
    "positions" DROP COLUMN "unrealized_pnl_sat";
