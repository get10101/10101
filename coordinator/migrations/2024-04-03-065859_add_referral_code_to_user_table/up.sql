ALTER TABLE users
    ADD COLUMN referral_code TEXT NOT NULL GENERATED ALWAYS AS (
        UPPER(RIGHT(pubkey, 6))
        ) STORED UNIQUE;
ALTER TABLE users
    ADD COLUMN used_referral_code TEXT;

CREATE TABLE referral_tiers (
    id SERIAL PRIMARY KEY,
    tier_level INTEGER NOT NULL,
    min_users_to_refer INTEGER NOT NULL,
    min_volume_per_referral INTEGER NOT NULL,
    fee_rebate REAL NOT NULL,
    number_of_trades INTEGER NOT NULL,
    active BOOLEAN NOT NULL
);

INSERT INTO referral_tiers (tier_level, min_users_to_refer, min_volume_per_referral, fee_rebate, number_of_trades, active)
VALUES
    (0, 0, 0, 0.0, 0, true),
    (1, 5, 1000, 0.25, 10, true),
    (2, 10, 5000, 0.40, 10, true),
    (3, 20, 10000, 0.60, 10, true);

CREATE VIEW user_referral_summary_view AS
SELECT u2.pubkey AS referring_user,
       u1.used_referral_code as referring_user_referral_code,
       u1.pubkey AS referred_user,
       u1.referral_code AS referred_user_referral_code,
       u1.contact,
       u1.nickname,
       u1.timestamp,
       COALESCE(SUM(t.quantity), 0) AS referred_user_total_quantity
FROM users u1
         JOIN users u2 ON u1.used_referral_code = u2.referral_code
         LEFT JOIN trades t ON t.trader_pubkey = u1.pubkey
GROUP BY u1.pubkey, u1.referral_code, u2.pubkey, u1.used_referral_code, u1.contact, u1.nickname, u1.timestamp;
