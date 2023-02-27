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

`bash just coordinator`

or in short

```bash
just c
```

## Regtest environment

The following command will start a regtest bitcoin node, electrs, chopsticks, esplora, lnd node and a http server allowing you to faucet your bitcoin and lightning wallet.

```bash
docker-compose up
```

### How to faucet your lightning wallet.

#### Setup

1. Start the coordinator with `cargo run --bin coordinator` or `just coordinator`
2. Open `http://localhost:8080/faucet/` (note: ensure to add the trailing `/` as otherwise nginx will try to redirect the call)
3. Ensure you have enough balance on your bitcoin wallet. Hit the mine button a couple of times if not.
4. Get a new address of your coordinator by running `curl http://localhost:8000/api/newaddress`
5. Faucet some coins to your coordinator wallet. Hit the mine button afterwards so the transaction gets into a block.
6. Open `http://localhost:8080/channel/` (note: ensure to add the trailing `/` as otherwise nginx will try to redirect the call)
7. Copy the address of the lnd node and faucet that wallet as described in step 5.
8. Copy the node id (_pubkey@host:port_) from your coordinator logs and instruct lnd to open a channel with your coordinator. Set a reasonable channel capacity. Note, this capacity will be only inbound for your coordinator.
9. Mine a few blocks (at least 6) so that the channel gets announced.

#### Fauceting your lightning wallet

10. Create an invoice in your 10101 app by navigating to the receive screen.
11. Copy the invoice and enter it on the lightning faucet. Hit send and you will receive your funds momentarily.
