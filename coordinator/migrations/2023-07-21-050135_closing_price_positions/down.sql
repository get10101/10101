-- This file should undo anything in `up.sql`
ALTER TABLE
    "positions" DROP COLUMN "closing_price";
