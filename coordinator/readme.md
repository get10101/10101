# Coordinator

## Orderbook

The orderbook is a simple webservice with CRUD functionality over orders, i.e.
it offers

- `HTTP::GET /orders`: to retrieve all orders
- `HTTP::POST /orders`: to create a new order
- `HTTP::UPDATE /orders`: to update an order
- `HTTP::DELETE /orders`: to delete an order

## Run

```bash
cargo run --bin orderbook
```

## Development

### Install diesel cli

```bash
cargo install diesel_cli --no-default-features --features postgres,sqlite
```

### Spin up a postgres db

Run this from the project root to start the database

```bash
docker compose up -d db
```

### Setup diesel

The db settings are currently hardcoded in main.rs:

```
postgres://postgres:mysecretpassword@localhost:5432/orderbook
```

Run this from the `coordinator` dir:

```bash
diesel setup --database-url=postgres://postgres:mysecretpassword@localhost:5432/orderbook --migration-dir ./migrations
```

```bash
diesel migration run --database-url=postgres://postgres:mysecretpassword@localhost:5432/orderbook --migration-dir ./migrations
```

To re-run (i.e. revert all and then run all) migrations you can use:

```bash
diesel migration redo --all --database-url=postgres://postgres:mysecretpassword@localhost:5432/orderbook --migration-dir ./migrations
```
