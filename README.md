# 10101 (_Ten-Ten-One_) - Decentralised finance. For real.

<img href="https://matrix.to/#/#tentenone:matrix.org" alt="10101 dev chat" src="https://img.shields.io/matrix/tentenone%3Amatrix.org">

<p align="center">
  <img height="300" src="./logos/1500x1500.png" alt="10101 Logo">
</p>

10101 combines the power of a self-custodial on-chain and off-chain wallet with the vast world of trading. 10101 - a numeral palindrome and the binary representation of 21 - as in 21 million possible bitcoin. The vision of 10101 embodies what Bitcoin stands for: Decentralized and censorship resistant money.

## Getting Started

To begin, ensure that you have a working installation of the following items:

- [Docker](https://docs.docker.com/) and docker-compose
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

## Contributing

We encourage community contributions whether it be a bug fix or an improvement to the documentation.
Please have a look at the [contributing guidelines](./CONTRIBUTING.md).

## Dependencies

A lot of complexity for building the app has been encapsulated in a [just](justfile)-file.
You can install `just` with `cargo install just`.

To see the available commands with explanations, simply run `just --list`.

To install necessary project dependencies for all targets, run the following:

```sh
just deps
```

### Diesel database dependencies

Some crates, e.g. the coordinator use [`diesel`](https://diesel.rs/guides/getting-started) for the database connection.
This may require installing dependencies, such as e.g. `libpql` for the postgres database for the coordinator.

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

## Running 10101

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

### Run the mobile-app on the Android simulator

Note: Ensure that the Android simulator is running on your machine so it can be selected as target.
Also ensure that you have run `just deps-android` to install the right targets for build.

The following command will build and start all the necessary services, including the android app.

```bash
just all-android
```

### How to faucet your wallet.

You can test the app on `regtest`, which means that the wallet needs to be fauceted with the provided steps before you can start trading.

#### Setup

1. If you have run the app before, we recommend to clear the dev environment by running `just wipe`

2. Start the complete project stack with `just all`.

#### Adding funds to 10101 wallet

1. Navigate to the receive screen.

2. Double-click on the generated QR code and click the "Pay with 10101 faucet" button. Note, if you do not specify an amount by
   default 1 BTC will sent to your regtest wallet.

#### Useful information for local regtest debugging

1. Follow coordinator's logs - `tail -f data/coordinator/regtest.log`
2. Block explorer - http://localhost:8080/
3. Bitcoin faucet - http://localhost:8080/faucet/

## Deploy for android beta

1. Create a `./mobile/android/key.properties` with the content from the key generation step from here https://docs.flutter.dev/deployment/android#signing-the-app
2. `just clean` never hurts but might not be necessary ;)
3. `just gen`
4. `just android-release`
5. `NETWORK=regtest just build-android-app-bundle`
6. `NETWORK=regtest just upload-app-bundle`

TL;DR;
a shortcut for this is available but it is recommended to execute each step separately:

```bash
just release-app-bundle-regtest
```
