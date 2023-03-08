-- Your SQL goes here
CREATE TYPE "Direction_Type" AS ENUM ('long', 'short');
CREATE TABLE "orders" (
    id SERIAL PRIMARY KEY,
    price REAL NOT NULL,
    maker_id TEXT NOT NULL,
    taken BOOLEAN NOT NULL DEFAULT FALSE,
    direction "Direction_Type" NOT NULL,
    quantity REAL NOT NULL
)
