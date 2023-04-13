CREATE TABLE IF NOT EXISTS payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    payment_hash TEXT UNIQUE NOT NULL,
    preimage TEXT,
    secret TEXT,
    htlc_status TEXT NOT NULL,
    amount_msat BIGINT,
    flow TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
)
