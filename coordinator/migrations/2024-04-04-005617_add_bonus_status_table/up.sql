-- Your SQL goes here

CREATE TABLE IF NOT EXISTS bonus_status (
    id SERIAL PRIMARY KEY,
    trader_pubkey TEXT NOT NULL,
    tier_level INTEGER NOT NULL,
    fee_rebate REAL NOT NULL DEFAULT 0.0,
    bonus_type "BonusStatus_Type" NOT NULL,
    activation_timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    deactivation_timestamp TIMESTAMP WITH TIME ZONE NOT NULL
);
