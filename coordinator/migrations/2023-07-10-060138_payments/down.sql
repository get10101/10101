-- This file should undo anything in `up.sql`
DROP INDEX IF EXISTS payments_payment_hash ON payments(payment_hash);
DROP TABLE "payments";
