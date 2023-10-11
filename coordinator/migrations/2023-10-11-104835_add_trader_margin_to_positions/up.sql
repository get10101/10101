ALTER TABLE positions
    ADD trader_margin  BIGINT NULL;

UPDATE positions set trader_margin = CAST(quantity AS FLOAT)  / (CAST(average_entry_price AS FLOAT) * CAST(trader_leverage as FLOAT)) * 100000000;

ALTER TABLE positions
ALTER COLUMN trader_margin SET NOT NULL;