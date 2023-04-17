CREATE TABLE "users" (
    id SERIAL PRIMARY KEY NOT NULL,
    pubkey TEXT NOT NULL,
    email TEXT NOT NULL,
    nostr TEXT NOT NULL
);
