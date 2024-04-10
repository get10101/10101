-- Must use `IF NOT EXISTS` because enum values cannot be removed on "down" migrations.
ALTER TYPE "Protocol_Type_Type"
      ADD VALUE IF NOT EXISTS 'resize-position';

ALTER TYPE "Protocol_Type_Type"
      RENAME VALUE 'open' TO 'open-channel';

ALTER TYPE "Protocol_Type_Type"
      RENAME VALUE 'renew' TO 'open-position';
