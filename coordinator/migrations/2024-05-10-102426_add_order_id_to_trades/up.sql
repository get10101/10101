-- Your SQL goes here
ALTER TABLE orders
    RENAME COLUMN trader_order_id TO order_id;

ALTER TABLE orders
    RENAME COLUMN trader_id TO trader_pubkey;

ALTER TABLE trade_params
    ADD COLUMN order_id UUID REFERENCES orders(order_id);

ALTER TABLE trades
    ADD COLUMN order_id UUID REFERENCES orders(order_id);
