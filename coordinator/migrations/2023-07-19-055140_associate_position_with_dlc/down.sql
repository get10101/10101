-- This file should undo anything in `up.sql`
ALTER TABLE
    "positions" DROP COLUMN "temporary_contract_id";
