ALTER TABLE
    orders
ADD COLUMN order_expiry_timestamp BIGINT NOT NULL DEFAULT 0;
