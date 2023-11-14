ALTER TABLE trades
ADD COLUMN dlc_expiry_timestamp timestamp WITH TIME ZONE;

ALTER TYPE "PositionState_Type"
ADD VALUE IF NOT EXISTS 'Resizing';
