-- This file should undo anything in `up.sql`
ALTER TABLE
    trades RENAME COLUMN collateral TO our_collateral;
