ALTER TABLE positions
ADD COLUMN order_matching_fees BIGINT NOT NULL DEFAULT 0;

-- To avoid division by zero.
UPDATE positions
SET order_matching_fees = 0 where average_entry_price=0;

-- Before this migration, the taker fee was hard-coded to 0.30%.
UPDATE positions
SET order_matching_fees = quantity * (1 / average_entry_price) * 0.0030 where average_entry_price!=0;
