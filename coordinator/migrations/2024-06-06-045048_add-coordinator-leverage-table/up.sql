CREATE TABLE coordinator_leverages
(
    id                   SERIAL PRIMARY KEY,
    trader_leverage      INT NOT NULL,
    coordinator_leverage INT NOT NULL
);

INSERT INTO coordinator_leverages (trader_leverage, coordinator_leverage)
VALUES (1, 1),
       (2, 2),
       (3, 3),
       (4, 4),
       (5, 5);
