<p align="center">
  <img height="300" src="./logos/logo.png">
</p>

# 10101 - One app, all things bitcoin

10101 combines the power of a self-custodial on-chain and off-chain wallet with the vast world of trading. 10101 - a numeral palindrome and the binary representation of 21 - as in 21 million possible bitcoin. The vision of 10101 embodies what Bitcoin stands for: Decentralized and censorship resistant money.

## Getting Started

To begin, ensure that you have a working installation of the following items:

- [Flutter SDK](https://docs.flutter.dev/get-started/install)
- [Rust language](https://rustup.rs/)
- Appropriate [Rust targets](https://rust-lang.github.io/rustup/cross-compilation.html) for cross-compiling to your device
- For Android targets:
  - Install [cargo-ndk](https://github.com/bbqsrc/cargo-ndk#installing)
  - Install Android NDK 22, then put its path in one of the `gradle.properties`, e.g.:

```
echo "ANDROID_NDK=.." >> ~/.gradle/gradle.properties
```

## Dependencies

A lot of complexity for building the app has been encapsulated in a [just](justfile)-file.
You can install `just` with `cargo install just`.
To see the available commands, simply run `make --list`.

To install necessary project dependencies for all targets, run the following:

```sh
just deps
```

It is also important to run the following to generate the Flutter-Rust glue code:

```sh
just gen
```

### Diesel database dependencies

Some crates, e.g. the coordinator use [`diesel`](https://diesel.rs/guides/getting-started) for the database connection.
This may require installing dependencies, such as e.g. `libpql` for the postgres database for the coordinator.

If you run into linking troubles when trying to build the coordinator you might have to configure the linker as so:

```bash
RUSTFLAGS='-L /path/to/libpq/lib' cargo build --bin coordinator
```

Alternatively you can configure this flag in `.cargo/config.toml` so you don't have to configure it all the time:

```toml
[target.aarch64-apple-darwin]
rustflags = '-L /path/to/libpq/lib'
```

where `/path/to` is the path to `libpq` on your machine.

## Mobile App

### Run the mobile-app natively (on your Linux/MacOS/other OS)

```bash
just deps
```

Note: it is not necessary to run this everytime again

```bash
just native
```

```bash
just run
```

### Run the mobile-app on the iOS simulator

Note: Ensure that the iOS simulator is running on your machine so it can be selected as target.

```bash
just deps
```

Note: it is not necessary to run this everytime again

```bash
just ios
```

```bash
just run
```

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

## Coordinator

### Run the coordinator

In order to successfully run the coordinator you will have to provide the coordinator with a PostgreSQL database.
The easiest way to do so is by starting the [local regest dev environemnt](#development-environment) through `docker-compose up`.

`bash just coordinator`

or in short

```bash
just c
```

## Development environment

The docker development environment provides the managed database containers as well as a regest bitcoin setup.

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

### How to faucet your lightning wallet.

#### Setup

1. Start the coordinator with `cargo run --bin coordinator` or `just coordinator`.

   _Ensure that you are using your network ip address and not localhost. This is critical as the docker container will otherwise not be able to reach the coordinator._
2. Open `http://localhost:8080/faucet/` (note: ensure to add the trailing `/` as otherwise nginx will try to redirect the call)
3. Ensure you have enough balance on your bitcoin wallet. Hit the mine button a couple of times if not.
4. Get a new address of your coordinator by running `curl http://localhost:8000/api/newaddress`
5. Faucet some coins to your coordinator wallet. Hit the mine button afterwards so the transaction gets into a block.
6. Open `http://localhost:8080/channel/` (note: ensure to add the trailing `/` as otherwise nginx will try to redirect the call)
7. Copy the address of the lnd node and faucet that wallet as described in step 5.
8. Open a channel with your coordinator (02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9@[coordinator ip]:9045) and set a reasonable channel capacity.

   _Note, if you're on mac or windows you can use `host.docker.internal` as coordinator ip._
9. Mine a few blocks (at least 6) so that the channel gets announced.

#### Fauceting your lightning wallet

10. Create an invoice in your 10101 app by navigating to the receive screen.

    _Note, that you have to provide the coordinator host to the mobile app like that `just run`_
11. Copy the invoice and enter it on the lightning faucet. Hit send and you will receive your funds momentarily.
