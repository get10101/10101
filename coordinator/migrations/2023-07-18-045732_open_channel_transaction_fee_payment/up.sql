-- Your SQL goes here
ALTER TABLE
    channels
ADD
    COLUMN open_channel_fee_payment_hash TEXT REFERENCES payments(payment_hash);
CREATE INDEX IF NOT EXISTS channels_funding_txid ON channels(funding_txid);
