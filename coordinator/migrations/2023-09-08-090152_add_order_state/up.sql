-- Your SQL goes here
CREATE TYPE "OrderState_Type" AS ENUM (
    'Open',
    'Matched',
    'Taken',
    'Failed'
);

ALTER TABLE "orders" ADD COLUMN "order_state" "OrderState_Type" NOT NULL DEFAULT 'Open';

UPDATE orders SET order_state = 'Taken' WHERE taken = true;
UPDATE orders SET order_state = 'Failed' WHERE taken = false and order_type = 'market';
