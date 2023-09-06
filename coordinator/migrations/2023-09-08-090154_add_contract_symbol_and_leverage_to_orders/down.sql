-- This file should undo anything in `up.sql`
ALTER TABLE orders
    DROP COLUMN "contract_symbol",
    DROP COLUMN "leverage";
