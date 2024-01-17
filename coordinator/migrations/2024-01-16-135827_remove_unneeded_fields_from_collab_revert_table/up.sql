-- Your SQL goes here
ALTER TABLE collaborative_reverts DROP COLUMN "funding_txid";
ALTER TABLE collaborative_reverts DROP COLUMN "funding_vout";
-- we do not store channels at the moment, hence, we can't have the reference to channels
ALTER TABLE collaborative_reverts
    DROP CONSTRAINT IF EXISTS collaborative_reverts_channel_id_fkey;
