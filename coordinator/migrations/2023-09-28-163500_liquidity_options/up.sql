CREATE TABLE "liquidity_options" (
       id SERIAL PRIMARY KEY NOT NULL,
       rank SMALLINT NOT NULL,
       title TEXT NOT NULL,
       trade_up_to_sats BIGINT NOT NULL,
       min_deposit_sats BIGINT NOT NULL,
       max_deposit_sats BIGINT NOT NULL,
       min_fee_sats BIGINT DEFAULT 0,
       fee_percentage FLOAT NOT NULL,
       coordinator_leverage REAL NOT NULL,
       active BOOLEAN NOT NULL DEFAULT TRUE,
       created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
       updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO liquidity_options (rank, title, trade_up_to_sats, max_deposit_sats, min_deposit_sats, min_fee_sats, fee_percentage, coordinator_leverage, active) VALUES (1, 'Large', 3000000, 3000000, 750000, 10000, 1.0, 2.0, true);
INSERT INTO liquidity_options (rank, title, trade_up_to_sats, max_deposit_sats, min_deposit_sats, min_fee_sats, fee_percentage, coordinator_leverage, active) VALUES (2, 'Medium', 1500000, 1500000, 375000, 10000, 1.0, 2.0, true);
INSERT INTO liquidity_options (rank, title, trade_up_to_sats, max_deposit_sats, min_deposit_sats, min_fee_sats, fee_percentage, coordinator_leverage, active) VALUES (3, 'Small', 200000, 200000, 50000, 10000, 1.0, 2.0, true);
