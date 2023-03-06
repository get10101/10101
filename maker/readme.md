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
