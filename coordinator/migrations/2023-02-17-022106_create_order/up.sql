-- Your SQL goes here
CREATE TYPE "Direction_Type" AS ENUM ('long', 'short');
CREATE TABLE "orders" (
    id SERIAL PRIMARY KEY NOT NULL,
    trader_order_id UUID UNIQUE NOT NULL,
    price REAL NOT NULL,
    maker_id TEXT NOT NULL,
    taken BOOLEAN NOT NULL DEFAULT FALSE,
    direction "Direction_Type" NOT NULL,
    quantity REAL NOT NULL,
    timestamp timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX trader_order_id ON orders(trader_order_id);
