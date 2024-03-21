# Additional information for developers

This document specifies how to start up `10101` as a developer and how to run the development environment on your local machine.
For getting a better understanding of the software architecture decisions you can refer to our [architecture decision records (ADRs)](/docs/readme.adoc).
Aimed primarily for developers.

## 10101 app

### Flutter Rust Bridge

10101 uses Rust and Flutter, and leverages [flutter-rust-bridge](https://github.com/fzyzcjy/flutter_rust_bridge) to generate code.

Whenever anything in the API changes, one must re-generate the FFI code with the following command:

-`sh -just gen -`

### Native build

To build the native version of the app, run:

```sh
just native
```

### iOS build

```sh
just ios
```

### Android build

```
just android
```

### Run

After building the app, one can run the Flutter app by typing:

```sh
just run
```

Note: 10101 app requires all the other services (e.g. Docker setup, coordinator, maker) running, otherwise it may not function properly.

## Coordinator

### Run the coordinator

In order to successfully run the coordinator you will have to provide the coordinator with a PostgreSQL database.
The easiest way to do so is by starting the [local regtest dev environemnt](#development-environment) through `docker compose up --build`. The `--build` ensures that all tables exist for `maker` and `coordinator`.

`bash just coordinator`

or in short

```bash
just c
```

## Maker

### Run the maker

To run the coordinator you will need a PostgeSQL database.

The easiest way to do so is by starting the [local regtest dev environemnt](#development-environment) through `docker compose up --build`. The `--build` ensures that all tables exist for `maker` and `coordinator`.

```bash
just maker
```

## Development environment

The docker development environment provides the managed database containers as well as a regtest bitcoin setup.

For more information on what containers are available please have a look at the [docker-compose](docker-compose.yml) file.
To start the development environment you can just run:

```bash
docker compose up
```

You can add `-d` to run the environment in the background.
Please refer to the [docker](https://docs.docker.com/) docs for more information on how to use docker / docker-compose.

Our development environment is based on [nigiri](https://github.com/vulpemventures/nigiri).
If you are a `nigiri` user, make sure to stop it before running the `10101` docker-compose setup, and that you prune the Docker containers by running:

```bash
nigiri stop
docker container prune
```

Otherwise, you might have troubles starting 10101, due to port conflicts on containers.

### Mobile Tests

The flutter project contains flutter tests and tests in the native rust backend of the mobile app.

Run the flutter tests:

```
just flutter-test
```

Note that this command takes care of re-generating the generated [`mockito`](https://pub.dev/packages/mockito) mocks before running the test.

Run the native rust backend tests:

```
just native-test
```

#### Resetting dev environment

In order to wipe all the runtime data, run:

```sh
just wipe
```

Wiping (resetting) the data will:

- stop Docker containers remove all Docker volumes
- clear `coordinator` data (except the default seed)
- remove 10101 native app data from the native

iOS/Android app data should be cleared either on device itself, as they can't be easily scripted.

#### Resetting iOS Simulator app

Note: iOS Simulator has a CLI interface to automate this, but the device identifier is unique.

In order to find out the iOS Simulator device identifier, one can run:

```sh
xcrun simctl list
```

In order to wipe device

```sh
xcrun simctl erase $DEVICE_IDENTIFIER
```

### Diesel database dependencies

Some crates, e.g. the coordinator use [`diesel`](https://diesel.rs/guides/getting-started) for the database connection.
This may require installing dependencies, such as e.g. `libpql` for the postgres database for the coordinator.

If you run into linking troubles when trying to build the coordinator you might have to configure the linker as so:

```bash
RUSTFLAGS='-L /path/to/libpq/lib' cargo build --bin coordinator
```

Alternatively you can configure this flag in `~/.cargo/config.toml` so you don't have to configure it all the time:

```toml
[target.aarch64-apple-darwin]
rustflags = '-L /path/to/libpq/lib'
```

where `/path/to` is the path to `libpq` on your machine.
