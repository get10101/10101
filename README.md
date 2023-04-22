<p align="center">
  <img height="300" src="./logos/1500x1500.png" alt="10101 Logo">
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

- For iOS targets:
  - XCode
  - Cocoapods

You can see whether you have all the sufficient dependencies for your platform by running `flutter doctor`.

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

### How to faucet your lightning wallet.

The app currently works only on `regtest`, which means that the wallet needs to be fauceted with the provided steps before you can start trading.

#### Setup

0. If you have run the app before, we recommend to clear the dev environment by running `just wipe`

1. Start the complete project stack with `just all`.

2. Fund and configure coordinator by running `just fund`

#### Adding funds to 10101 lightning wallet

3. Create an invoice in your 10101 app by navigating to the receive screen.
   _Note, that you have to provide the coordinator host to the mobile app like that `just run`_

4. Open `http://localhost:8080/faucet/` (note: ensure to add the trailing `/` as otherwise nginx will try to redirect the request)

5. Copy the invoice and enter it on the lightning faucet. Hit send and you will receive your funds momentarily.
