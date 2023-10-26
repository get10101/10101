-- defaults to genesis block tx id
ALTER TABLE "collaborative_reverts"
    ADD funding_txid TEXT NOT NULL DEFAULT '4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b';
ALTER TABLE "collaborative_reverts"
    ADD funding_vout  INT NOT NULL DEFAULT 0;
