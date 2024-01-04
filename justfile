# To use this file, install Just: cargo install just
set dotenv-load
line_length := "100"
coordinator_log_file := "$PWD/data/coordinator/regtest.log"
maker_log_file := "$PWD/data/maker/regtest.log"

# location of pubspec
pubspec := "$PWD/mobile/pubspec.yaml"

# public regtest constants
public_regtest_coordinator := "03507b924dae6595cfb78492489978127c5f1e3877848564de2015cd6d41375802@34.32.0.52:9045"
public_regtest_esplora := "http://34.32.0.52:3000"
public_coordinator_http_port := "80"
public_regtest_oracle_endpoint := "http://34.32.0.52:8081"
public_regtest_oracle_pk := "5d12d79f575b8d99523797c46441c0549eb0defb6195fe8a080000cbe3ab3859"

# command to get the local IP of this machine
get_local_ip := if os() == "linux" {
 "ip -o route get to 1 | sed -n 's/.*src \\([0-9.]\\+\\).*/\\1/p'"
} else if os() == "macos" {
 "ipconfig getifaddr en0"
} else {
 "echo 'Only linux and macos are supported';
 exit"
}

# RUST_LOG is overriden for FRB codegen invocations it if RUST_LOG isn't info or debug, which means
# a command like `RUST_LOG="warn" just all` would fail
rust_log_for_frb := if env_var_or_default("RUST_LOG", "") =~ "(?i)(trace)|(debug)" {
    "debug"
} else {
    "info"
}


default: gen
precommit: gen lint

# Install missing dependencies.
deps: deps-gen deps-android deps-ios

deps-gen:
    cargo install flutter_rust_bridge_codegen@1.78.0

# Install dependencies for Android (build targets and cargo-ndk)
deps-android:
    cargo install cargo-ndk
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android

# Install dependencies for iOS
deps-ios:
    cargo install cargo-lipo
    rustup target add aarch64-apple-ios x86_64-apple-ios

gen:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd mobile
    flutter pub get
    RUST_LOG={{ rust_log_for_frb }} flutter_rust_bridge_codegen \
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
android args="":
    cd mobile/native && cargo ndk -t armeabi-v7a -t arm64-v8a -t x86_64 -t x86 -o ../android/app/src/main/jniLibs build {{args}}

# Note that this does not include x86_64 unlike the above, as android x86_64 is only a development
# target and not a deployment target
android-release:
    cd mobile/native && cargo ndk -t armeabi-v7a -t arm64-v8a -o ../android/app/src/main/jniLibs build --release --verbose

# Build Rust library for iOS (debug mode)
ios:
    cd mobile/native && CARGO_TARGET_DIR=../../target/ios_debug cargo lipo
    cp target/ios_debug/universal/debug/libnative.a mobile/ios/Runner

# Build Rust library for iOS (release mode)
ios-release:
    cd mobile/native && cargo lipo --release
    cp target/universal/release/libnative.a mobile/ios/Runner


run args="":
    #!/usr/bin/env bash
    cd mobile && \
      flutter run {{args}} \
      --dart-define="COMMIT=$(git rev-parse HEAD)" \
      --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
      --dart-define="REGTEST_FAUCET=http://localhost:8080" \
      --dart-define="REGTEST_MAKER_FAUCET=http://localhost:18000/api/pay-invoice" \
      --dart-define="HEALTH_CHECK_INTERVAL_SECONDS=2"

# Run against our public regtest server
run-regtest args="":
    #!/usr/bin/env bash
    cd mobile && \
      flutter run {{args}} \
        --dart-define="COMMIT=$(git rev-parse HEAD)" \
        --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
        --dart-define="ESPLORA_ENDPOINT={{public_regtest_esplora}}" \
        --dart-define="COORDINATOR_P2P_ENDPOINT={{public_regtest_coordinator}}" \
        --dart-define="COORDINATOR_PORT_HTTP={{public_coordinator_http_port}}" \
        --dart-define="ORACLE_ENDPOINT={{public_regtest_oracle_endpoint}}" \
        --dart-define="ORACLE_PUBKEY={{public_regtest_oracle_pk}}"

# Run against our public mainnet server
run-mainnet args="":
    #!/usr/bin/env bash
    cd mobile && \
      flutter run {{args}} \
        --dart-define="COMMIT=$(git rev-parse HEAD)" \
        --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
        --dart-define="ESPLORA_ENDPOINT=http://api.10101.finance:3000" \
        --dart-define="COORDINATOR_P2P_ENDPOINT=022ae8dbec1caa4dac93f07f2ebf5ad7a5dd08d375b79f11095e81b065c2155156@46.17.98.29:9045" \
        --dart-define="COORDINATOR_PORT_HTTP=80" \
        --dart-define="ORACLE_ENDPOINT=http://oracle.10101.finance" \
        --dart-define="NETWORK=mainnet" \
        --dart-define="ORACLE_PUBKEY=93051f54feefdb4765492a85139c436d4857e2e331a360c89a16d6bc02ba9cd0" \
        --dart-define="RGS_SERVER_URL=https://rapidsync.lightningdevkit.org/snapshot"

# Specify correct Android flavor to run against our public regtest server
run-regtest-android args="":
    #!/usr/bin/env bash
    cd mobile && \
      flutter run {{args}} \
        --dart-define="COMMIT=$(git rev-parse HEAD)" \
        --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
        --dart-define="ESPLORA_ENDPOINT={{public_regtest_esplora}}" \
        --dart-define="COORDINATOR_P2P_ENDPOINT={{public_regtest_coordinator}}" \
        --dart-define="COORDINATOR_PORT_HTTP={{public_coordinator_http_port}}" \
        --dart-define="ORACLE_ENDPOINT={{public_regtest_oracle_endpoint}}" \
        --dart-define="ORACLE_PUBKEY={{public_regtest_oracle_pk}}" \
        --flavor test

[unix]
run-local-android args="":
    #!/usr/bin/env bash
    LOCAL_IP=$({{get_local_ip}})
    echo "Android app will connect to $LOCAL_IP for 10101 services"
    cd mobile && \
      flutter run {{args}} \
        --dart-define="COMMIT=$(git rev-parse HEAD)" \
        --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
        --dart-define="ESPLORA_ENDPOINT=http://${LOCAL_IP}:3000" \
        --dart-define="COORDINATOR_P2P_ENDPOINT=02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9@${LOCAL_IP}:9045" \
        --dart-define="REGTEST_FAUCET=http://${LOCAL_IP}:8080" \
        --dart-define="REGTEST_MAKER_FAUCET=http://${LOCAL_IP}:18000/api/pay-invoice" \
        --dart-define="COORDINATOR_PORT_HTTP=8000" \
        --dart-define="ORACLE_ENDPOINT=http://${LOCAL_IP}:8081" \
        --dart-define="ORACLE_PUBKEY=16f88cf7d21e6c0f46bcbc983a4e3b19726c6c98858cc31c83551a88fde171c0" \
        --flavor local

fund args="":
      cargo run -p fund --example fund

# Fund remote regtest instance
fund-regtest:
    cargo run -p tests-e2e --example fund -- --faucet=http://34.32.0.52:8080 --coordinator=http://34.32.0.52:80

clean:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd mobile
    rm -rf mobile/android/app/src/main/jniLibs/*
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

    # Locate macOS 10101 application directory if it exists
    if [ -d "$HOME/Library/Containers/" ]; then
        FOUND_DIR=""
        for dir in $HOME/Library/Containers/*/Data/Library/Application\ Support/finance.get10101.app; do
            if [ -d "$dir" ]; then
                FOUND_DIR="$dir"
                break  # Exit loop after the first match. Remove if you want all matches.
            fi
        done
        # default to dummy dir if nothing got found to ensure nothing unnecessary gets deleted
        MACOS_PATH="${FOUND_DIR:-/path/to/dummy/directory}"
    fi

    # If no path was found, use a dummy path to avoid errors
    if [[ -z ${MACOS_PATH+x} || ! $MACOS_PATH || ! ${MACOS_PATH//[[:space:]]/} ]]; then
        echo "no macos path found, setting dummy value"
        MACOS_PATH="/path/to/dummy/directory"
    fi

    # Array of possible app data directories (OS dependent)
    directories=(
      "$HOME/Library/Containers/finance.get10101.app/Data/Library/Application Support/finance.get10101.app"
      "$HOME/Library/Containers/finance.get10101.app/"
      "$MACOS_PATH"
      "$HOME/.local/share/finance.get10101.app/"
    )
    # Remove all possible app data directories
    for dir in "${directories[@]}"
    do
        if [[ -d "$dir" ]]; then
            echo "App data directory ${dir} exists, removing it now..."
            rm -r "$dir"
        else
            echo "$dir not found, skipping..."
        fi
    done
    echo "Done wiping 10101 app"


lint: lint-flutter clippy

clippy:
    cd mobile/native && just cargo-clippy
    cd coordinator && just cargo-clippy
    cd maker && just cargo-clippy
    for crate in crates/*; do (cd "${crate}" && echo "Running clippy on ${crate}" && just cargo-clippy); done

[private]
cargo-clippy:
    cargo clippy --all-targets -- -D warnings

lint-flutter:
    cd mobile && flutter analyze --fatal-infos .

alias flutter-lint := lint-flutter

alias fmt := format
format: dprint flutter-format

dprint:
    dprint fmt

# Flutter lacks a dprint plugin, use its own formatter
flutter-format:
    cd mobile && dart format . --fix --line-length {{line_length}}

alias c := coordinator
coordinator args="":
    #!/usr/bin/env bash
    set -euxo pipefail

    settings_target_path="data/coordinator/regtest/coordinator-settings.toml"

    if [ ! -f "$settings_target_path" ]; then
        cp coordinator/example-settings/test-coordinator-settings.toml "$settings_target_path"
        echo "Copied test settings to $(pwd)/$settings_target_path"
    else
        echo "Using preexisting settings file at $(pwd)/$settings_target_path"
    fi

    cargo run --bin coordinator -- {{args}}

maker args="":
    cargo run --bin maker -- {{args}}

flutter-test:
    cd mobile && flutter pub run build_runner build --delete-conflicting-outputs && flutter test

# Tests for the `native` crate
native-test:
    cd mobile/native && cargo test

test: flutter-test native-test

# Run expensive tests from the `ln-dlc-node` crate.
ln-dlc-node-test args="": docker
    # wait a few seconds to ensure that Docker containers started
    sleep 2
    # adjust the max amount of available file descriptors - we're making a lot of requests, and it might go over the limit
    ulimit -n 1024
    RUST_BACKTRACE=1 cargo test -p ln-dlc-node -- --ignored --test-threads=1 {{args}}

# Runs background Docker services
docker:
    docker-compose up -d

docker-logs:
    docker-compose logs

# Starts coordinator process in the background, piping logs to a file (used in other recipes)
run-coordinator-detached:
    #!/usr/bin/env bash
    set -euxo pipefail

    just wait-for-electrs-to-be-ready

    echo "Starting (and building) coordinator"
    just coordinator &> {{coordinator_log_file}} &
    just wait-for-coordinator-to-be-ready
    echo "Coordinator successfully started. You can inspect the logs at {{coordinator_log_file}}"

# Starts maker process in the background, piping logs to a file (used in other recipes)
run-maker-detached:
    #!/usr/bin/env bash
    set -euxo pipefail

    just wait-for-electrs-to-be-ready

    echo "Starting (and building) maker"
    cargo run --bin maker &> {{maker_log_file}} &
    just wait-for-maker-to-be-ready
    echo "Maker successfully started. You can inspect the logs at {{maker_log_file}}"

# Attach to the current coordinator logs
coordinator-logs:
    #!/usr/bin/env bash
    set -euxo pipefail
    less +F {{coordinator_log_file}}

# Attach to the current maker logs
maker-logs:
    #!/usr/bin/env bash
    set -euxo pipefail
    less +F {{maker_log_file}}

# Run services in the background
services: docker run-coordinator-detached run-maker-detached fund

# Run everything at once (docker, coordinator, native build)
# Note: if you have mobile simulator running, it will start that one instead of native, but will *not* rebuild the mobile rust library.
all args="": services gen native
    #!/usr/bin/env bash
    set -euxo pipefail
    just run "{{args}}"

# Run everything at once, tailored for iOS development
all-ios: services gen ios run

# Run iOS on public regtest (useful for device testing, where local regtest is not available)
ios-regtest: gen ios run-regtest

# Run everything at once, tailored for Android development (rebuilds Android)
all-android: services gen android run-local-android

[private]
wait-for-electrs-to-be-ready:
    #!/usr/bin/env bash
    set +e

    check_if_electrs_is_ready ()
    {
        docker logs electrs 2>&1 | grep "Electrum RPC server running" > /dev/null
        return $?
    }

    while true
    do
        if check_if_electrs_is_ready; then
            echo "Electrum server is ready"
            break
        else
            echo "Waiting for Electrum server to be ready"
            sleep 1
        fi
    done

[private]
wait-for-coordinator-to-be-ready:
    #!/usr/bin/env bash
    set +e

    endpoint="http://localhost:8000/api/newaddress"
    max_attempts=600
    sleep_duration=1

    check_endpoint() {
      response=$(curl -s -o /dev/null -w "%{http_code}" "$endpoint")
      if [ "$response" -eq 200 ]; then
        echo "Coordinator is ready!"
        exit 0
        else
        echo "Coordinator not ready yet. Retrying..."
        return 1
        fi
        }

    attempt=1
    while [ "$attempt" -le "$max_attempts" ]; do
      if check_endpoint; then
        exit 0
        fi

      sleep "$sleep_duration"
      attempt=$((attempt + 1))
      done

    echo "Max attempts reached. Coordinator is still not ready."
    exit 1

[private]
wait-for-maker-to-be-ready:
    #!/usr/bin/env bash
    set +e

    endpoint="http://localhost:18000/"
    max_attempts=600
    sleep_duration=1

    check_endpoint() {
      response=$(curl -s -o /dev/null -w "%{http_code}" "$endpoint")
      if [ "$response" -eq 200 ]; then
        echo "Maker is ready!"
        exit 0
        else
        echo "Maker not ready yet. Retrying..."
        return 1
        fi
        }

    attempt=1
    while [ "$attempt" -le "$max_attempts" ]; do
      if check_endpoint; then
        exit 0
        fi

      sleep "$sleep_duration"
      attempt=$((attempt + 1))
      done

    echo "Max attempts reached. Maker is still not ready."
    exit 1

build-ipa args="":
    #!/usr/bin/env bash
    BUILD_NUMBER=$(git rev-list HEAD --count)
    args=()

    if [ "$NETWORK" = "regtest" ]; then
      args+=(--flavor test)
    fi

    cd mobile && flutter build ipa "${args[@]}" \
           --dart-define="ESPLORA_ENDPOINT=${ESPLORA_ENDPOINT}" \
           --dart-define="COORDINATOR_P2P_ENDPOINT=${COORDINATOR_P2P_ENDPOINT}" \
           --dart-define="NETWORK=${NETWORK}" \
           --dart-define="COMMIT=$(git rev-parse HEAD)" \
           --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
           --dart-define="COORDINATOR_PORT_HTTP=${COORDINATOR_PORT_HTTP}" \
           --dart-define="ORACLE_ENDPOINT=${ORACLE_ENDPOINT}" \
           --dart-define="ORACLE_PUBKEY=${ORACLE_PUBKEY}" \
           --dart-define="RGS_SERVER_URL=${RGS_SERVER_URL}" \
           --build-number=${BUILD_NUMBER} \
           {{args}}

publish-testflight:
    cd mobile && xcrun altool --upload-app --type ios --file ./build/ios/ipa/10101.ipa --apiKey ${ALTOOL_API_KEY} --apiIssuer ${ALTOOL_API_ISSUER}

build-ipa-no-codesign: (build-ipa "--no-codesign")

publish-testflight-fastlane:
    cd mobile/ios/fastlane && bundle exec fastlane closed_beta --verbose

release-testflight: gen ios build-ipa publish-testflight

version:
    cargo --version && rustc --version && flutter --version

build-apk-regtest:
    #!/usr/bin/env bash
    BUILD_NAME=$(yq -r .version {{pubspec}})
    BUILD_NUMBER=$(git rev-list HEAD --count)
    echo "build name: ${BUILD_NAME}"
    echo "build number: ${BUILD_NUMBER}"
    cd mobile && flutter build apk  \
      --build-name=${BUILD_NAME} \
      --build-number=${BUILD_NUMBER} \
      --release \
      --dart-define="COMMIT=$(git rev-parse HEAD)" \
      --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
      --dart-define="ESPLORA_ENDPOINT={{public_regtest_esplora}}" \
      --dart-define="COORDINATOR_P2P_ENDPOINT={{public_regtest_coordinator}}" \
      --dart-define="COORDINATOR_PORT_HTTP={{public_coordinator_http_port}}" \
      --dart-define="ORACLE_ENDPOINT={{public_regtest_oracle_endpoint}}" \
      --dart-define="ORACLE_PUBKEY={{public_regtest_oracle_pk}}" \
      --flavor demo

release-apk-regtest: gen android-release build-apk-regtest

build-app-bundle-regtest:
    #!/usr/bin/env bash
    BUILD_NAME=$(yq -r .version {{pubspec}})
    BUILD_NUMBER=$(git rev-list HEAD --count)
    echo "build name: ${BUILD_NAME}"
    echo "build number: ${BUILD_NUMBER}"
    cd mobile && flutter build appbundle \
        --build-name=${BUILD_NAME} \
        --build-number=${BUILD_NUMBER} \
        --release \
        --dart-define="COMMIT=$(git rev-parse HEAD)" \
        --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
        --dart-define="ESPLORA_ENDPOINT={{public_regtest_esplora}}" \
        --dart-define="COORDINATOR_P2P_ENDPOINT={{public_regtest_coordinator}}" \
        --dart-define="COORDINATOR_PORT_HTTP={{public_coordinator_http_port}}" \
        --dart-define="ORACLE_ENDPOINT={{public_regtest_oracle_endpoint}}" \
        --dart-define="ORACLE_PUBKEY={{public_regtest_oracle_pk}}" \
        --flavor demo


build-android-app-bundle:
    #!/usr/bin/env bash
    BUILD_NAME=$(yq -r .version {{pubspec}})
    BUILD_NUMBER=$(git rev-list HEAD --count)
    echo "build name: ${BUILD_NAME}"
    echo "build number: ${BUILD_NUMBER}"

    flavor_arg=()
    if [ "$NETWORK" = "regtest" ]; then
      flavor_arg+=(--flavor demo)
    else
      flavor_arg+=(--flavor full)
    fi

    # Replacing package id using the env variable.
    os={{os()}}
    echo "building on '$os' for '$NETWORK'"

    cd mobile && flutter build appbundle  \
      --build-name=${BUILD_NAME} \
      --build-number=${BUILD_NUMBER} \
      --release \
      --dart-define="ESPLORA_ENDPOINT=${ESPLORA_ENDPOINT}" \
      --dart-define="COORDINATOR_P2P_ENDPOINT=${COORDINATOR_P2P_ENDPOINT}" \
      --dart-define="NETWORK=${NETWORK}" \
      --dart-define="COMMIT=$(git rev-parse HEAD)" \
      --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
      --dart-define="COORDINATOR_PORT_HTTP=${COORDINATOR_PORT_HTTP}" \
      --dart-define="ORACLE_ENDPOINT=${ORACLE_ENDPOINT}" \
      --dart-define="ORACLE_PUBKEY=${ORACLE_PUBKEY}" \
      --dart-define="RGS_SERVER_URL=${RGS_SERVER_URL}" \
       "${flavor_arg[@]}"

upload-app-bundle:
    #!/usr/bin/env bash

    cd mobile/android/fastlane

    if [ "$NETWORK" = "regtest" ]; then
      echo "Uploading for regtest"
      ANDROID_PACKAGE_NAME='finance.get10101.app.demo' FASTLANE_ANDROID_APP_SCHEME='demo' bundle exec fastlane alpha
    else
      echo "Uploading for mainnet"
      ANDROID_PACKAGE_NAME='finance.get10101.app' FASTLANE_ANDROID_APP_SCHEME='full' bundle exec fastlane internal
    fi

release-app-bundle-regtest: gen android-release build-app-bundle-regtest upload-app-bundle

# Run prometheus for local debugging (needs it installed, e.g. `brew install prometheus`)
prometheus:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd services/prometheus
    prometheus

# Reset gathered prometheus metrics
wipe-prometheus:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd services/prometheus
    rm -rf data


alias e2e := tests-e2e
# end-to-end tests
tests-e2e args="": services
    #!/usr/bin/env bash
    set -euxo pipefail
    RUST_BACKTRACE=1 cargo test -p tests-e2e -- --ignored --test-threads=1 {{args}}

# Run a single end-to-end test for debugging purposes
e2e-single test_name="": services
    #!/usr/bin/env bash
    set -euxo pipefail
    RUST_BACKTRACE=1 cargo test -p tests-e2e --test {{test_name}} -- --ignored --nocapture

# Run database migrations for the app
migrate-app:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd mobile/native
    export DATABASE_URL="sqlite://app.sql"
    diesel setup
    echo "Running migrations for the app"
    diesel migration run
    rm app.sql
    echo "Done."

# Run database migrations for the coordinator
# note: requires postgresql to be running
migrate-coordinator: docker
    #!/usr/bin/env bash
    set -euxo pipefail
    cd coordinator
    export DATABASE_URL="postgres://postgres:mysecretpassword@localhost:5432"
    diesel setup
    echo "Running migrations for the coordinator"
    diesel migration run
    echo "Done."

# Re-run database migrations for both app and coordinator
migrate: migrate-app migrate-coordinator

dart: flutter-format lint-flutter

# Check whether your dev environment is compatible with the project
doctor:
    #!/usr/bin/env bash
    echo "Checking your dev environment for compatibility with building 10101."
    ./check_compatibility.sh

# vim:expandtab:sw=4:ts=4
