CREATE TABLE "channel_opening_params" (
    order_id TEXT PRIMARY KEY NOT NULL,
    coordinator_reserve BIGINT NOT NULL,
    trader_reserve BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);
