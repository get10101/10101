-- Your SQL goes here
CREATE TABLE "orders" (
    id SERIAL PRIMARY KEY,
    price INTEGER NOT NULL,
    maker_id TEXT NOT NULL,
    taken BOOLEAN NOT NULL DEFAULT FALSE
)
