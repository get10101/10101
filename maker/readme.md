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

- `HTTP-POST api/channel`: allows you to open a channel with a target node.

Below a snippet to opena channel with the coordinator

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
