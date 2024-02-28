
CREATE TYPE "Protocol_State_Type" AS ENUM ('Pending', 'Success', 'Failed');

CREATE TABLE "dlc_protocols"
(
    id                      SERIAL                              PRIMARY KEY NOT NULL,
    protocol_id             UUID                                UNIQUE NOT NULL,
    previous_protocol_id    UUID                                REFERENCES dlc_protocols (protocol_id),
    channel_id              TEXT                                NOT NULL,
    contract_id             TEXT                                NOT NULL,
    protocol_state          "Protocol_State_Type"               NOT NULL,
    trader_pubkey           TEXT                                NOT NULL REFERENCES users (pubkey),
    timestamp               timestamp WITH TIME ZONE            NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE "trade_params"
(
    id                      SERIAL                              PRIMARY KEY NOT NULL,
    protocol_id             UUID                                NOT NULL REFERENCES dlc_protocols(protocol_id),
    trader_pubkey           TEXT                                NOT NULL,
    quantity                REAL                                NOT NULL,
    leverage                REAL                                NOT NULL,
    average_price           REAL                                NOT NULL,
    direction               "Direction_Type"                    NOT NULL
);
