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
cargo run
```

The webinterface will be reachable under `http://localhost:3001`
