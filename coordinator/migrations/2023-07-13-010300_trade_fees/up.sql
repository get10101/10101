-- Your SQL goes here
-- insert a payment with amount zero that is a dummy default payment that corresponds to this actual zero-amount invoice:
-- lnbcrt0m1pj26wczdq50fjhymeqd9h8vmmfvdjsnp4qtwk40kf07d8fzlhdt2s9vqyeczarvk37safua4a0kz7wellkq3vjpp5d7dce9wzhfa3s4a3n7t4xu3ss9slah6sl6mc5ffzqqf45sv82ggqsp54cmyvxklz48ap60guqsln4wuh2ranlap3grg2djt5ykz7005fc6s9qyysgqcqpcxq8z7ps9nqepcz2l2s4qrp6qeks68qry55ylcz542c8a6m6fffmhlmhtn9nnf458ngv83pk0473xfecdk7m7maumqk4jvaymdg7fpgn9tujcehxpqqte97xd
-- We insert this dummy payment so we are able to sum up fees on already existing trades and make the fee payment hash a mandatory field.
INSERT INTO
    payments (
        payment_hash,
        htlc_status,
        amount_msat,
        flow,
        payment_timestamp,
        description
    )
VALUES
    (
        '6f9b8c95c2ba7b1857b19f975372308161fedf50feb78a252200135a41875210',
        'Succeeded',
        0,
        'Inbound',
        CURRENT_TIMESTAMP,
        'zero amount payment dummy default value'
    );
ALTER TABLE
    trades
ADD
    COLUMN "fee_payment_hash" TEXT NOT NULL DEFAULT '6f9b8c95c2ba7b1857b19f975372308161fedf50feb78a252200135a41875210' REFERENCES payments(payment_hash);
