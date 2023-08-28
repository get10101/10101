-- This file should undo anything in `up.sql`
-- Note: There is no down migration for removing the `Announced variant that was added to `ChannelState_Type` because it is not feasible to remove enum variants in the db!
ALTER TABLE
    transactions DROP COLUMN "raw";
