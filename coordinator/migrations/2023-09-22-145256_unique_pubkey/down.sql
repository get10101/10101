-- This file should undo anything in `up.sql`
ALTER TABLE users
    DROP CONSTRAINT unique_pubkey;
