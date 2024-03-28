ALTER TABLE "trades"
    DROP COLUMN IF EXISTS "order_matching_fee_sat",
    ADD COLUMN "fee_payment_hash" TEXT NOT NULL DEFAULT '6f9b8c95c2ba7b1857b19f975372308161fedf50feb78a252200135a41875210' REFERENCES payments(payment_hash);
