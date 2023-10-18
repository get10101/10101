-- Your SQL goes here
ALTER TABLE "orders"
    ADD COLUMN "stable" BOOLEAN NOT NULL DEFAULT false;

ALTER TABLE "positions"
    ADD COLUMN "stable" BOOLEAN NOT NULL DEFAULT false;
