-- This file should undo anything in `up.sql`
ALTER TABLE orders
    DROP COLUMN "order_reason";

DROP TYPE "OrderReason_Type";
