-- Your SQL goes here
ALTER TYPE "PositionState_Type"
    ADD
    VALUE IF NOT EXISTS 'Rollover';
