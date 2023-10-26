ALTER TABLE collaborative_reverts
    DROP COLUMN IF EXISTS funding_txid;
ALTER TABLE collaborative_reverts
    DROP COLUMN IF EXISTS funding_vout;
