
CREATE TYPE "Protocol_Type_Type" AS ENUM ('open', 'renew', 'settle', 'close', 'force-close', 'rollover');

ALTER TABLE dlc_protocols ADD COLUMN "protocol_type" "Protocol_Type_Type" NOT NULL DEFAULT 'open';
