-- This file should undo anything in `up.sql`
ALTER TABLE orders
    DROP COLUMN "stable";

ALTER TABLE positions
    DROP COLUMN "stable";
