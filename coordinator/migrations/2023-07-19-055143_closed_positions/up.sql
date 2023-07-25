-- Your SQL goes here
-- Note that the `IF NOT EXISTS` is essential because there is no `down` migration for removing this value because it is not really feasible to remove enum values!
-- In order to allow re-running this migration we thus have to make sure to only add the value if it does not exist yet.
ALTER TYPE "PositionState_Type"
ADD
    VALUE IF NOT EXISTS 'Closed';
ALTER TABLE
    positions
ADD
    COLUMN "realized_pnl_sat" BIGINT;
