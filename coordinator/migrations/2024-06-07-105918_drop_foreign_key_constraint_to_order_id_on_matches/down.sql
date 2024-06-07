ALTER TABLE "matches"
    ADD CONSTRAINT matches_order_id_fkey
        FOREIGN KEY (order_id)
            REFERENCES orders (order_id);

ALTER TABLE "matches"
    ADD CONSTRAINT matches_match_order_id_fkey
        FOREIGN KEY (match_order_id)
            REFERENCES orders (order_id);
