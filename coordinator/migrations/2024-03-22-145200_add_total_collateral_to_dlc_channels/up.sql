ALTER TABLE "dlc_channels"
    ADD COLUMN coordinator_funding_sats BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN trader_funding_sats BIGINT NOT NULL DEFAULT 0;
