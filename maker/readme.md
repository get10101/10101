# Maker

Make sure to add a new DB to postgres.
If you use our local dev setup, you cas use.
This will rebuild the db contianer and add the missing database.

```bash
docker compose up db --build
```

```bash
cargo run --bin maker
```

## API

The maker has a useful API for channel management,
e.g.

- `HTTP-POST api/channels`: allows you to open a channel with a target node.

Below a snippet to open a channel with the coordinator

```bash
curl -d '{
            "target": {
              "pubkey" : "02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9",
              "address" : "127.0.0.1:9045"
            },
            "local_balance": 100000,
            "remote_balance": 0
         }' -H "Content-Type: application/json"  \
         -X POST http://localhost:18000/api/channels
```

- `HTTP-GET api/channels`: list all channels (usable and not yet usable)

- `HTTP-POST api/pay-invoice`: pays a provided invoice, e.g.:

```bash
curl -X POST http://localhost:18000/api/pay-invoice/lnbcrt10u1pjqvlzydq8w3jhxaqpp5t96ysv9a8xh056r3y9w4qczxwcu469vq0tr3mm7240adynz9nhdqsp5pjy2ks5j0a8yxpk3gtwaagsc5ygst4d2yf3pumdmghwe2njy0vds9qrsgqcqpcrzjqtwk40kf07d8fzlhdt2s9vqyeczarvk37safua4a0kz7wellkq3vjqqqqyqqn8cqqyqqqqlgqqqyugqq9g6ugm5r29uktn6x2lf0s9edgrjy2tvun283l8v0laaxcd87ga2505mq0ax5mak2f4kn87l7ans7j6xl7fj2cwlyt27jufcghptdxv5fgpalze60
```
