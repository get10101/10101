-- This file should undo anything in `up.sql`
ALTER TABLE trade_params
    DROP COLUMN matching_fee;

