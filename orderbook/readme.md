# Orderbook

The orderbook is a simple webservice with CRUD functionality over orders, i.e.
it offers

- `HTTP::GET /orders`: to retrieve all orders
- `HTTP::POST /orders`: to create a new order
- `HTTP::UPDATE /orders`: to update an order
- `HTTP::DELETE /orders`: to delete an order

## Run

To run the orderbook, first make sure that `Rocket.toml` is up2date and the database is running.
Then run

```bash
cargo run --bin orderbook
```

## Development

### Install diesel cli

```bash
cargo install diesel_cli --no-default-features --features postgres,sqlite
```

### Spin up a postgres db

```bash
docker-compose up -d
```

### Setup diesel

To tell diesel where our db is, export this var into Rocket.toml

```toml
[default.databases.postgres_database]
url = "postgres://postgres:mysecretpassword@localhost:5432/orderbook"
```

```bash
diesel setup
```

```bash
diesel migration generate create_posts
```
