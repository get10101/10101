CREATE TABLE "polls_whitelist" (
    id              SERIAL PRIMARY KEY NOT NULL,
    poll_id         SERIAL REFERENCES polls (id) NOT NULL,
    trader_pubkey   TEXT REFERENCES users (pubkey) NOT NULL
);
ALTER TABLE "polls" ADD COLUMN whitelisted BOOLEAN NOT NULL DEFAULT FALSE;
