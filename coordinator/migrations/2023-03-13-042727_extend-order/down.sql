-- This file should undo anything in `up.sql`
ALTER TABLE
    orders RENAME COLUMN trader_id TO maker_id;
ALTER TABLE
    orders DROP COLUMN IF EXISTS "order_type";
DROP TYPE IF EXISTS "OrderType_Type";
