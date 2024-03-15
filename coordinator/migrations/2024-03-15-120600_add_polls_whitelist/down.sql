DROP TABLE "polls_whitelist";
ALTER TABLE "polls" DROP COLUMN IF EXISTS whitelisted;
