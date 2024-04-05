DROP VIEW IF EXISTS user_referral_summary_view;

ALTER TABLE users
    DROP COLUMN referral_code;
ALTER TABLE users
    DROP COLUMN used_referral_code;

DROP TABLE IF EXISTS bonus_tiers;
DROP TYPE IF EXISTS "BonusStatus_Type";

