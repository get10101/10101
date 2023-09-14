-- Your SQL goes here
ALTER TABLE
    orders
    ADD
        COLUMN "reason" TEXT NOT NULL DEFAULT 'Manual';
