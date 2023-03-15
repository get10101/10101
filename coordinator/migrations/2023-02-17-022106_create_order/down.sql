-- This file should undo anything in `up.sql`
DROP TABLE "orders";
DROP TYPE IF EXISTS "Direction_Type";
DROP INDEX IF EXISTS trader_order_id;
