-- Your SQL goes here
CREATE TABLE IF NOT EXISTS "routing_fees" (
    id SERIAL PRIMARY KEY NOT NULL,
    amount_msats BIGINT NOT NULL,
    -- We save the previous and next channel id because this is the only information we get (if set) when forwarding. This could be useful to understand routing fee constraints better in the future.
    prev_channel_id TEXT,
    next_channel_id TEXT,
    -- We only have a created_at timestamp because entries in this table should not be updated
    created_at timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
