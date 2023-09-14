-- This file should undo anything in `up.sql`
ALTER TABLE "orders"
    ADD COLUMN "taken" BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE orders SET taken = true WHERE order_state = 'Taken';
