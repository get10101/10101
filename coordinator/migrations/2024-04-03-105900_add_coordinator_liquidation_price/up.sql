ALTER TABLE "positions"
    ADD COLUMN coordinator_liquidation_price REAL NOT NULL DEFAULT 0;

UPDATE positions SET coordinator_liquidation_price = average_entry_price * coordinator_leverage / (coordinator_leverage + 1) where trader_direction='short';
UPDATE positions SET coordinator_liquidation_price = average_entry_price * coordinator_leverage / (coordinator_leverage - 1) where trader_direction='long' and coordinator_leverage > 1;
UPDATE positions SET coordinator_liquidation_price = 1048575 where trader_direction='long' and coordinator_leverage = 1;

