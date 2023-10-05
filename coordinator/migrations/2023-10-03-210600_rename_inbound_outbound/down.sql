-- This file should undo anything in `up.sql`
ALTER TABLE channels RENAME COLUMN inbound_sats TO inbound;
ALTER TABLE channels RENAME COLUMN outbound_sats TO outbound;
