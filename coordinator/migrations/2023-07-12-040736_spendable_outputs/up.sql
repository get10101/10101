CREATE TABLE "spendable_outputs" (
    id SERIAL PRIMARY KEY NOT NULL,
    -- hex encoded
    txid TEXT NOT NULL,
    vout int NOT NULL,
    -- hex representation of LDK's own encoding
    descriptor TEXT NOT NULL
);
