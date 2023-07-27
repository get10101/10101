-- Your SQL goes here
ALTER TABLE
    channels
ADD
    COLUMN "fake_scid" TEXT UNIQUE;
CREATE INDEX IF NOT EXISTS channels_fake_scid ON channels(fake_scid);
ALTER TYPE "ChannelState_Type"
ADD
    VALUE IF NOT EXISTS 'Announced';
