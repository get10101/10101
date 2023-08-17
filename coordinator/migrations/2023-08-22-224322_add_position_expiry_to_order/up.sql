-- add proposed position expiry column to orders table
ALTER TABLE
    orders
ADD
    COLUMN position_expiry TIMESTAMP WITH TIME ZONE NOT NULL;
UPDATE
    orders
SET
    position_expiry = NOW() 
WHERE
    expiry IS NULL;
