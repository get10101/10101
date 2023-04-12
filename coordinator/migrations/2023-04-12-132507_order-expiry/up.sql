-- Your SQL goes here
-- Your SQL goes here
ALTER TABLE
    orders
ADD
    COLUMN expiry TIMESTAMP WITH TIME ZONE NOT NULL;
UPDATE
    orders
SET
    expiry = NOW()
WHERE
    expiry IS NULL;
