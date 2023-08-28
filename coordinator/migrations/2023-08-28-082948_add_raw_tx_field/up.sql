-- Your SQL goes here
ALTER TABLE
    transactions
    ADD
        COLUMN "raw" TEXT NOT NULL DEFAULT 'undefined';
