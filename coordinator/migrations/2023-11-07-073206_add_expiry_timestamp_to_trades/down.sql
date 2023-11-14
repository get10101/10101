ALTER TABLE trades
DROP COLUMN dlc_expiry_timestamp;

-- Note: There is no down migration for removing the `Resizing`
-- variant that was added to `PositionState_Type` because it is not
-- feasible to remove enum variants in the db!
