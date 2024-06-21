ALTER TABLE trades DROP COLUMN IF EXISTS order_id;

ALTER TABLE trade_params DROP COLUMN IF EXISTS order_id;

ALTER TABLE orders
    RENAME COLUMN trader_pubkey TO trader_id;

ALTER TABLE orders
    RENAME COLUMN order_id TO trader_order_id;

