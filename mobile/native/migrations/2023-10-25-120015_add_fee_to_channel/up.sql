-- Your SQL goes here
ALTER TABLE "channels" ADD COLUMN "fee_sats" BIGINT;
ALTER TABLE "channels" ADD COLUMN "open_channel_payment_hash" TEXT;

ALTER TABLE "payments" ADD COLUMN "funding_txid" TEXT;
