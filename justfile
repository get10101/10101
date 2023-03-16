# To use this file, install Just: cargo install just
line_length := "100"
coordinator_log_file := "$PWD/data/coordinator/regtest.log"
maker_log_file := "$PWD/data/maker/regtest.log"

default: gen
precommit: gen lint

# deps: Install missing dependencies.
deps: deps-gen deps-android deps-ios

deps-gen:
    cargo install flutter_rust_bridge_codegen

# deps-android: Install dependencies for Android (build targets and cargo-ndk)
deps-android:
    cargo install cargo-ndk
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android

# deps-ios: Install dependencies for iOS
deps-ios:
    cargo install cargo-lipo
    rustup target add aarch64-apple-ios x86_64-apple-ios

gen:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd mobile
    flutter pub get
    flutter_rust_bridge_codegen \
        --rust-input native/src/api.rs \
        --c-output ios/Runner/bridge_generated.h \
        --extra-c-output-path macos/Runner/ \
        --rust-output native/src/bridge_generated/bridge_generated.rs \
        --dart-output lib/bridge_generated/bridge_generated.dart \
        --dart-decl-output lib/bridge_generated/bridge_definitions.dart \
        --dart-format-line-length {{line_length}}

native:
    cd mobile/native && cargo build

# Build Rust library for Android native targets
android:
    cd mobile/native && cargo ndk -o ../android/app/src/main/jniLibs build

# ios: Build Rust library for iOS
ios:
    cd mobile/native && cargo lipo
    cp target/universal/debug/libnative.a mobile/ios/Runner


run args="":
    cd mobile && flutter run {{args}}

fund:
    cargo run --example fund

clean:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd mobile
    flutter clean
    cd native && cargo clean

# Wipes everything
wipe: wipe-docker wipe-coordinator wipe-maker wipe-app

wipe-docker:
    #!/usr/bin/env bash
    set -euxo pipefail
    docker-compose down -v

wipe-coordinator:
    pkill -9 coordinator && echo "stopped coordinator" || echo "coordinator not running, skipped"
    rm -rf data/coordinator/regtest
    git checkout data/coordinator

wipe-maker:
    #!/usr/bin/env bash
    set -euxo pipefail
    pkill -9 maker && echo "stopped maker" || echo "maker not running, skipped"
    rm -rf data/maker/regtest

wipe-app:
    #!/usr/bin/env bash
    set -euxo pipefail
    echo "Wiping native 10101 app"
    # Array of possible app data directories (OS dependent)
    directories=(
      "$HOME/Library/Containers/finance.get10101.app/Data/Library/Application Support/finance.get10101.app"
      "$HOME/.local/share/10101/"
    )
    # Remove all possible app data directories
    for dir in "${directories[@]}"
    do
        if [ -d "$dir" ]; then
            echo "App data directory ${dir} exists, removing it now..."
            rm -r "$dir"
        else
            echo "$dir not found, skipping..."
        fi
    done
    echo "Done wiping 10101 app"


lint: lint-flutter clippy

clippy:
    cd mobile/native && cargo clippy --all-targets -- -D warnings
    cd coordinator && cargo clippy --all-targets -- -D warnings

lint-flutter:
    cd mobile && flutter analyze --fatal-infos .

alias fmt := format
format: dprint flutter-format

dprint:
    dprint fmt

# Flutter lacks a dprint plugin, use its own formatter
flutter-format:
    cd mobile && dart format . --fix --line-length {{line_length}}

alias c := coordinator
coordinator args="":
    cargo run --bin coordinator -- {{args}}

maker args="":
    cargo run --bin maker -- {{args}}

flutter-test:
    cd mobile && flutter pub run build_runner build && flutter test

native-test:
    cd mobile/native

test: flutter-test native-test

# Runs background Docker services
docker:
     docker-compose up -d

docker-logs:
     docker-compose logs

# Starts coordinator process in the background, piping logs to a file (used in other recipes)
run-coordinator-detached:
    #!/usr/bin/env bash
    set -euxo pipefail
    echo "Starting (and building) coordinator"
    cargo run --bin coordinator &> {{coordinator_log_file}} &
    echo "Coordinator successfully started. You can inspect the logs at {{coordinator_log_file}}"

# Starts maker process in the background, piping logs to a file (used in other recipes)
run-maker-detached:
    #!/usr/bin/env bash
    set -euxo pipefail
    echo "Starting (and building) maker"
    cargo run --bin maker &> {{maker_log_file}} &
    echo "Maker successfully started. You can inspect the logs at {{maker_log_file}}"

# Attach to the current coordinator logs
coordinator-logs:
    #!/usr/bin/env bash
    set -euxo pipefail
    tail -f {{coordinator_log_file}}

# Attach to the current maker logs
maker-logs:
    #!/usr/bin/env bash
    set -euxo pipefail
    tail -f {{maker_log_file}}

# Run services in the background
services: docker run-coordinator-detached run-maker-detached

# Run everything at once (docker, coordinator, native build)
# Note: if you have mobile simulator running, it will start that one instead of native, but will *not* rebuild the mobile rust library.
all: services gen native run

# Run everything at once, tailored for iOS development (rebuilds iOS)
all-ios: services gen ios run

# vim:expandtab:sw=4:ts=4
