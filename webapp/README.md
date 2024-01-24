# A simple webfrontend to be self-hosted

## Run frontend only in dev mode

```bash
cd frontend
flutter run -d chrome
```

## Build the frontend to be served by rust

```bash
flutter build web
```

## Run the rust app

```bash
cargo run -- --cert-dir certs --data-dir ../data
```

The webinterface will be reachable under `https://localhost:3001`

Note: if you can't see anything, you probably forgot to run `flutter build web` before

## How to use curl with webapp

We need to store cookies between two curl calls, for that you can use the cookie jar of curl:

```bash
curl -b .cookie-jar.txt -c .cookie-jar.txt -X POST http://localhost:3001/api/login -d '{ "password": "satoshi" }' -H "Content-Type: application/json" -v
```

This will read and store the cookies in `.cookie-jar.txt`. So on the next call you can reference it the same way, e.g.

```bash
curl -b .cookie-jar.txt -c .cookie-jar.txt http://localhost:3001/api/balance
```
