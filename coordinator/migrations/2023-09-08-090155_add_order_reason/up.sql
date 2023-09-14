-- Your SQL goes here
CREATE TYPE "OrderReason_Type" AS ENUM (
    'Manual',
    'Expired'
);

ALTER TABLE "orders"
    ADD COLUMN "order_reason" "OrderReason_Type" NOT NULL DEFAULT 'Manual';
