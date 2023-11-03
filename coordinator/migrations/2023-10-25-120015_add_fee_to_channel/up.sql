-- Your SQL goes here
ALTER TABLE channels ADD COLUMN "fee_sats" BIGINT DEFAULT null;

ALTER TABLE
    channels DROP COLUMN open_channel_fee_payment_hash;
DROP INDEX IF EXISTS channels_funding_txid;