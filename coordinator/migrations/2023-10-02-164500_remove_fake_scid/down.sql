-- This file should undo anything in `up.sql`
ALTER TABLE channels DROP COLUMN liquidity_option_id;
ALTER TABLE channels ADD COLUMN "fake_scid" TEXT UNIQUE;
CREATE INDEX IF NOT EXISTS channels_fake_scid ON channels(fake_scid);
