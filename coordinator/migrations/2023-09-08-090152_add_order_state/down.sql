-- This file should undo anything in `up.sql`
ALTER TABLE
    orders DROP COLUMN "order_state";

DROP TYPE "OrderState_Type";
