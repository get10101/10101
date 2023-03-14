-- Your SQL goes here
CREATE TYPE "OrderType_Type" AS ENUM ('market', 'limit');
ALTER TABLE
    orders RENAME COLUMN maker_id TO trader_id;
ALTER TABLE
    orders
ADD
    COLUMN order_type "OrderType_Type" NOT NULL;
