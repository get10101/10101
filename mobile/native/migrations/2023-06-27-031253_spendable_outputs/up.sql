CREATE TABLE IF NOT EXISTS spendable_outputs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    -- formatted as <txid>:<vout>
    outpoint TEXT UNIQUE NOT NULL,
    -- hex representation of LDK's own encoding
    descriptor TEXT NOT NULL
)
