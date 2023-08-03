-- This file should undo anything in `up.sql`
-- Note: There is no down migration for removing the `Announced variant that was added to `ChannelState_Type` because it is not feasible to remove enum variants in the db!
ALTER TABLE
    channels DROP COLUMN "fake_scid";
DROP INDEX IF EXISTS channels_fake_scid;
