-- Note that the `IF NOT EXISTS` is essential because there is no `down` migration for removing this value because it is not really feasible to remove enum values!
-- In order to allow re-running this migration we thus have to make sure to only add the value if it does not exist yet.
ALTER TYPE "OrderType_Type"
ADD
    VALUE IF NOT EXISTS 'margin';

ALTER TABLE
    orders ADD COLUMN margin_sats REAL DEFAULT null;
