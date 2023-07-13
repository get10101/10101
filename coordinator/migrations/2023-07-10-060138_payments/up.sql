-- Your SQL goes here
CREATE TYPE "Payment_Flow_Type" AS ENUM ('Inbound', 'Outbound');
CREATE TYPE "Htlc_Status_Type" AS ENUM ('Pending', 'Succeeded', 'Failed');
CREATE TABLE IF NOT EXISTS "payments" (
    id SERIAL PRIMARY KEY NOT NULL,
    payment_hash TEXT UNIQUE NOT NULL,
    preimage TEXT,
    secret TEXT,
    htlc_status "Htlc_Status_Type" NOT NULL,
    amount_msat BIGINT,
    flow "Payment_Flow_Type" NOT NULL,
    payment_timestamp timestamp WITH TIME ZONE NOT NULL,
    created_at timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at timestamp WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    description TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS payments_payment_hash ON payments(payment_hash);
