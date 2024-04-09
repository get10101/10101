ALTER TABLE users
    ADD COLUMN referral_code TEXT NOT NULL GENERATED ALWAYS AS (
        UPPER(RIGHT(pubkey, 6))
        ) STORED UNIQUE;
ALTER TABLE users
    ADD COLUMN used_referral_code TEXT;

CREATE TYPE "BonusStatus_Type" AS ENUM ('Referral', 'Referent');

CREATE TABLE bonus_tiers (
    id SERIAL PRIMARY KEY,
    tier_level INTEGER NOT NULL,
    min_users_to_refer INTEGER NOT NULL,
    fee_rebate REAL NOT NULL,
    bonus_tier_type "BonusStatus_Type" NOT NULL,
    active BOOLEAN NOT NULL
);

INSERT INTO bonus_tiers (tier_level, min_users_to_refer, fee_rebate, bonus_tier_type, active)
VALUES (0, 0, 0.0, 'Referral', true),
       (1, 3, 0.20, 'Referral', true),
       (2, 5, 0.35, 'Referral', true),
       (3, 10, 0.50, 'Referral', true),
       (4, 0, 0.1, 'Referent', true);

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
