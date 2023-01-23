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

### Run the app natively (on your Linux/MacOS/other OS)

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

### Run the app on the iOS simulator

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
