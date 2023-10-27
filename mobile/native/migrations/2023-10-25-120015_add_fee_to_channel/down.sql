-- This file should undo anything in `up.sql`
ALTER TABLE "channels" DROP COLUMN "fee_sats";
ALTER TABLE "channels" DROP COLUMN "open_channel_payment_hash";

ALTER TABLE "payments" DROP COLUMN "funding_txid";
