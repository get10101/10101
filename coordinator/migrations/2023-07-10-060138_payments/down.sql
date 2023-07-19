-- This file should undo anything in `up.sql`
DROP INDEX IF EXISTS payments_payment_hash;
DROP TABLE "payments";
DROP TYPE "Payment_Flow_Type";
DROP TYPE "Htlc_Status_Type";
