-- This file should undo anything in `up.sql`
DELETE FROM
    payments
WHERE
    payment_hash = '6f9b8c95c2ba7b1857b19f975372308161fedf50feb78a252200135a41875210';
ALTER TABLE
    trades DROP COLUMN IF EXISTS "fee_payment_hash";
