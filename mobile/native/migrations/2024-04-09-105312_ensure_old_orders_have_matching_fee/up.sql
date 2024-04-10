-- Your SQL goes here
update orders
set matching_fee_sats = quantity / (execution_price * leverage) * 100000000 * 0.003
where matching_fee_sats IS NOT NULL
    and execution_price IS NOT NULL
    and matching_fee_sats IS NULL
    and state = 'filling'
   or 'filled';

-- if one of the values has not been set, we set this matching fee to 0.
update orders
set matching_fee_sats = 0
where matching_fee_sats IS NULL
   or execution_price IS NULL
    and matching_fee_sats IS NULL
    and state = 'filling'
   or 'filled';
