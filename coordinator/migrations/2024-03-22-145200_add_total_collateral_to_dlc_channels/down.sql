ALTER TABLE "dlc_channels"
    DROP COLUMN IF EXISTS coordinator_funding_sats,
    DROP COLUMN IF EXISTS trader_funding_sats;
