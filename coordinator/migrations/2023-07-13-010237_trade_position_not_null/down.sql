-- This file should undo anything in `up.sql`
ALTER TABLE
    trades
ALTER COLUMN
    position_id DROP NOT NULL;
