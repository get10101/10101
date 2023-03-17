# Additional information for developers

This document specifies how to start up `10101` as a developer and how to run the development environment on your local machine.
For getting a better understanding of the software architecture decisions you can refer to our [architecture decision records (ADRs)](/docs/readme.adoc).
Aimed primarily for developers.

## Getting started

10101 consists of sevaral systems:

- The `10101 app` that the trader uses for trading
- The `coordinator` that acts as a router for trade execution. The `coordinator` binary currently bundles the `orderbook`
- The `maker` that represents an automated market maker

All systems require `Rust` to be installed, the `10101 app` requires flutter for the mobile application.

### Language and Framework dependencies

To get going, ensure that you have a working installation of the following items:

- [Flutter SDK](https://docs.flutter.dev/get-started/install)
- [Rust language](https://rustup.rs/)
- Appropriate [Rust targets](https://rust-lang.github.io/rustup/cross-compilation.html) for cross-compiling to your device
- For Android targets:
  - Install [cargo-ndk](https://github.com/bbqsrc/cargo-ndk#installing)
  - Install Android NDK 22, then put its path in one of the `gradle.properties`, e.g.:

```
echo "ANDROID_NDK=.." >> ~/.gradle/gradle.properties
```

- For iOS targets:
  - XCode
  - Cocoapods

You can see whether you have all the sufficient dependencies for your platform by running `flutter doctor`.

### Just

A lot of complexity for building the app has been encapsulated in a [just](justfile)-file.
You can install `just` with `cargo install just`.

To see the available commands with explanations, simply run `just --list`.

To install necessary project dependencies for all targets, run the following:

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

#### MacOS

On macOS, one can install `libpq` with the following command:

```sh
brew install libpq
```

Bear in mind that `libpq` is keg-only (not installed globally). This means that you have to add the library path the linker manually.
The are a number ways to do that (e.g. by setting rustflags), however the easiest one is to add the following lines to your `.zshrc`/`.bashrc`

```sh
export LDFLAGS="-L/usr/local/opt/libpq/lib"
export CPPFLAGS="-I/usr/local/opt/libpq/include"
```

This will ensure that `libpq` is available during building the project

## Run the Dev Setup

The dev setup configures a `regtest` setup, which means that the wallet needs to be fauceted with the provided steps before you can start trading.
The justfile provides commands to automate starting the necessary docker containers, setting up the faucet as well as funding the coordinator.

### Getting Started

0. If you have run the app before, we recommend to clear the dev environment by running `just wipe`

1. Start the complete project stack with `just all`.

2. Fund and configure coordinator by running `just fund`

#### Adding funds to 10101 lightning wallet

3. Create an invoice in your 10101 app by navigating to the receive screen.
   _Note, that you have to provide the coordinator host to the mobile app like that `just run`_

4. Open `http://localhost:8080/faucet/` (note: ensure to add the trailing `/` as otherwise nginx will try to redirect the call)

5. Copy the invoice and enter it on the lightning faucet. Hit send and you will receive your funds momentarily.

### Run the app natively (on your Linux/MacOS/other OS)

The following command will build and start all the necessary services, including the native desktop 10101 app.

```bash
just all
```

### Run the mobile-app on the iOS simulator

Note: Ensure that the iOS simulator is running on your machine so it can be selected as target.

The following command will build and start all the necessary services, including the native desktop 10101 app.

```bash
just all-ios
```

### Resetting dev environment

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

### Development environment in depth

The docker development environment provides the managed database containers as well as a regtest bitcoin setup.

For more information on what containers are available please have a look at the [docker-compose](docker-compose.yml) file.
To start the development environment you can just run:

```bash
docker-compose up
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

### Flutter Rust Bridge

The 10101 app uses Rust and Flutter, and leverages [flutter-rust-bridge](https://github.com/fzyzcjy/flutter_rust_bridge) to generate code.

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

### Run app only

After building the app, one can run the Flutter app by typing:

```sh
just run
```

Note: 10101 app requires the other systems (coordinator, maker, ...) running, otherwise it may not function properly.

### Run coordinator and maker

To start just the coordinator you can use:

```bash
just coordinator
```

To start just the maker you can use:

```bash
just maker
```

Note that in order to successfully run the coordinator or maker you will have to provide the coordinator or maker with a PostgreSQL database.
If you are running the containers without using the just commands to start the dev setup it is recommended to run `docker-compose` with `docker-compose up --build`.
The `--build` ensures that all tables exist for `maker` and `coordinator`.
