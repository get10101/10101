-- Your SQL goes here
ALTER TABLE trade_params
    ADD COLUMN matching_fee BIGINT NOT NULL DEFAULT 0;
