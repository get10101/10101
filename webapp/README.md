# A simple web frontend to be self-hosted

## Run frontend only in dev mode

```bash
cd frontend
flutter run -d chrome
```

## Build the frontend to be served by Rust

```bash
flutter build web
```

## Run the Rust app

### With TLS

```bash
cargo run -- --cert-dir certs --data-dir ../data --secure
```

The web interface will be reachable under `https://localhost:3001`.

### Without TLS

```bash
cargo run -- --cert-dir certs --data-dir ../data
```

The web interface will be reachable under `http://localhost:3001`

### Troubleshooting

If you can't see anything, you probably forgot to run `flutter build web` before.

## How to interact with the backend with `curl`

We need to store cookies between `curl` calls. For that you can use the `curl`'s cookie jar:

```bash
curl -b .cookie-jar.txt -c .cookie-jar.txt -X POST http://localhost:3001/api/login -d '{ "password": "satoshi" }' -H "Content-Type: application/json" -v
```

This will read and store the cookies in `.cookie-jar.txt`. So on the next call you can reference it the same way:

```bash
curl -b .cookie-jar.txt -c .cookie-jar.txt http://localhost:3001/api/balance
```
