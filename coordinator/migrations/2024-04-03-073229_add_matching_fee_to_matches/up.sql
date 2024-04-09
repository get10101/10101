-- Your SQL goes here
alter table matches add column matching_fee_sats BIGINT NOT NULL DEFAULT 0;

update matches set matching_fee_sats=((quantity/execution_price) * 100000000)*0.003;
