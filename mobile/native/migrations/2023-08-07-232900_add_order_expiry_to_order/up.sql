ALTER TABLE
    orders
ADD COLUMN order_expiry_timestamp BIGINT NOT NULL DEFAULT (strftime('%s', 'now'));
