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
