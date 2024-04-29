CREATE TABLE funding_rates
(
    id SERIAL PRIMARY KEY NOT NULL,
    start_date TIMESTAMP WITH TIME ZONE NOT NULL,
    end_date TIMESTAMP WITH TIME ZONE NOT NULL,
    rate REAL NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE funding_fee_events
(
    id SERIAL PRIMARY KEY NOT NULL,
    amount_sats BIGINT NOT NULL,
    trader_pubkey TEXT NOT NULL,
    position_id INTEGER REFERENCES positions (id) NOT NULL,
    due_date TIMESTAMP WITH TIME ZONE NOT NULL,
    price REAL NOT NULL,
    funding_rate REAL NOT NULL,
    paid_date TIMESTAMP WITH TIME ZONE,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    -- To prevent generating duplicates for the same position.
    UNIQUE (position_id, due_date)
);

CREATE TABLE protocol_funding_fee_events
(
    id SERIAL PRIMARY KEY NOT NULL,
    protocol_id UUID REFERENCES dlc_protocols (protocol_id) NOT NULL,
    funding_fee_event_id INTEGER REFERENCES funding_fee_events (id) NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
