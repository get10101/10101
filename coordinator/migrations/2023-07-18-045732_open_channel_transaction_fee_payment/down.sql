-- This file should undo anything in `up.sql`
ALTER TABLE
    channels DROP COLUMN open_channel_fee_payment_hash;
DROP INDEX IF EXISTS channels_funding_txid;
