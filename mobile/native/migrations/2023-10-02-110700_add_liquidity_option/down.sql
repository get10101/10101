-- This file should undo anything in `up.sql`
ALTER TABLE
    channels DROP COLUMN "liquidity_option_id";
