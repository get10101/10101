-- This file should undo anything in `up.sql`
ALTER TABLE orders drop column matching_fee_sats;
