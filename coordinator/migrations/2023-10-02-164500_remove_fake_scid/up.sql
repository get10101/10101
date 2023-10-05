ALTER TABLE channels ADD COLUMN liquidity_option_id INTEGER;

ALTER TABLE channels DROP COLUMN "fake_scid";
DROP INDEX IF EXISTS channels_fake_scid;
