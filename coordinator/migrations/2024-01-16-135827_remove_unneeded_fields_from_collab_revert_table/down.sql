-- We delete all old collaborative revert entries - because we don't care about them anymore
DELETE FROM collaborative_reverts;

ALTER TABLE "collaborative_reverts"
    ADD funding_txid TEXT NOT NULL DEFAULT '4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b';
ALTER TABLE "collaborative_reverts"
    ADD funding_vout  INT NOT NULL DEFAULT 0;

ALTER TABLE collaborative_reverts
    ADD CONSTRAINT collaborative_reverts_channel_id_fkey
        FOREIGN KEY (channel_id) REFERENCES channels (channel_id);
