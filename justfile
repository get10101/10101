# To use this file, install Just: cargo install just
set dotenv-load
line_length := "100"
coordinator_log_file := "$PWD/data/coordinator/regtest.log"
maker_log_file := "$PWD/data/maker/regtest.log"

# location of pubspec
pubspec := "$PWD/mobile/pubspec.yaml"

# public regtest constants
public_regtest_coordinator := "03507b924dae6595cfb78492489978127c5f1e3877848564de2015cd6d41375802@35.189.57.114:9045"
public_regtest_esplora := "http://35.189.57.114:3000"
public_coordinator_http_port := "80"

# command to get the local IP of this machine
get_local_ip := if os() == "linux" {
 "ip -o route get to 1 | sed -n 's/.*src \\([0-9.]\\+\\).*/\\1/p'"
} else {
 "ifconfig | grep -Eo 'inet (addr:)?([0-9]*\\.){3}[0-9]*' | grep -Eo '([0-9]*\\.){3}[0-9]*' | grep -v '127.0.0.1'"
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
    cd mobile/native && cargo ndk -t armeabi-v7a -t arm64-v8a -o ../android/app/src/main/jniLibs build --release

# Build Rust library for iOS (debug mode)
ios:
    cd mobile/native && cargo lipo
    cp target/universal/debug/libnative.a mobile/ios/Runner

# Build Rust library for iOS (release mode)
ios-release:
    cd mobile/native && cargo lipo --release
    cp target/universal/release/libnative.a mobile/ios/Runner


run args="":
    #!/usr/bin/env bash
    cd mobile && flutter run {{args}} --dart-define="COMMIT=$(git rev-parse HEAD)" --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
    --dart-define="REGTEST_FAUCET=http://localhost:8080" --dart-define="HEALTH_CHECK_INTERVAL_SECONDS=2" \

# Run against our public regtest server
run-regtest args="":
    #!/usr/bin/env bash
    cd mobile && flutter run {{args}} --dart-define="COMMIT=$(git rev-parse HEAD)" --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
    --dart-define="ESPLORA_ENDPOINT={{public_regtest_esplora}}" --dart-define="COORDINATOR_P2P_ENDPOINT={{public_regtest_coordinator}}" \
    --dart-define="COORDINATOR_PORT_HTTP={{public_coordinator_http_port}}"

[unix]
run-local-android args="":
    #!/usr/bin/env bash
    LOCAL_IP=$({{get_local_ip}})
    echo "Android app will connect to $LOCAL_IP for 10101 services"
    cd mobile && flutter run {{args}} --dart-define="COMMIT=$(git rev-parse HEAD)" --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
    --dart-define="ESPLORA_ENDPOINT=http://${LOCAL_IP}:3000" --dart-define="COORDINATOR_P2P_ENDPOINT=02dd6abec97f9a748bf76ad502b004ce05d1b2d1f43a9e76bd7d85e767ffb022c9@${LOCAL_IP}:9045" \
    --dart-define="REGTEST_FAUCET=http://${LOCAL_IP}:8080" --dart-define="COORDINATOR_PORT_HTTP=8000" --flavor local

fund args="":
    #!/usr/bin/env bash
    BALANCE=$(curl -s localhost:8080/lnd/v1/balance/channels | sed 's/.*"balance":"\{0,1\}\([^,"]*\)"\{0,1\}.*/\1/')
    if [ $BALANCE -lt 10000000 ] || [ "{{args}}" = "--force" ] || [ "{{args}}" = "-f" ]
    then
      echo "Lightning faucet balance is $BALANCE; funding..."
      cargo run --example fund
    else
      echo "Lightning faucet balance is $BALANCE; skipping funding. Pass -f or --force to force."
    fi

# Fund remote regtest instance
fund-regtest:
    cargo run --example fund -- --faucet=http://35.189.57.114:8080 --coordinator=http://35.189.57.114:80

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
    # Array of possible app data directories (OS dependent)
    directories=(
      "$HOME/Library/Containers/finance.get10101.app/Data/Library/Application Support/finance.get10101.app"
      "$HOME/Library/Containers/finance.get10101.app/"
      "$HOME/.local/share/finance.get10101.app/"
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
    cd mobile/native && just cargo-clippy
    cd coordinator && just cargo-clippy
    cd maker && just cargo-clippy
    for crate in crates/*; do (cd "${crate}" && echo "Running clippy on ${crate}" && just cargo-clippy); done

[private]
cargo-clippy:
    cargo clippy --all-targets -- -D warnings

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
    cargo run --bin coordinator &> {{coordinator_log_file}} &
    just wait-for-coordinator-to-be-ready
    echo "Coordinator successfully started. You can inspect the logs at {{coordinator_log_file}}"

# Starts maker process in the background, piping logs to a file (used in other recipes)
run-maker-detached:
    #!/usr/bin/env bash
    set -euxo pipefail

    just wait-for-electrs-to-be-ready

    echo "Starting (and building) maker"
    cargo run --bin maker &> {{maker_log_file}} &
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
services: docker run-coordinator-detached run-maker-detached wait-for-coordinator-to-be-ready fund

# Run everything at once (docker, coordinator, native build)
# Note: if you have mobile simulator running, it will start that one instead of native, but will *not* rebuild the mobile rust library.
all: services gen native run

# Run everything at once, tailored for iOS development (rebuilds iOS)
all-ios: services gen ios run

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

build-ipa:
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
           --build-number=${BUILD_NUMBER}

publish-testflight:
    cd mobile && xcrun altool --upload-app --type ios --file ./build/ios/ipa/10101.ipa --apiKey ${ALTOOL_API_KEY} --apiIssuer ${ALTOOL_API_ISSUER}

release-testflight: gen ios build-ipa publish-testflight

version:
    cargo --version && rustc --version && flutter --version

build-apk-regtest:
    #!/usr/bin/env bash
    BUILD_NAME=$(yq -r .version {{pubspec}})
    BUILD_NUMBER=$(git rev-list HEAD --count)
    echo "build name: ${BUILD_NAME}"
    echo "build number: ${BUILD_NUMBER}"
    cd mobile && flutter build apk  --build-name=${BUILD_NAME} --build-number=${BUILD_NUMBER} --release --dart-define="COMMIT=$(git rev-parse HEAD)" --dart-define="BRANCH=$(git rev-parse --abbrev-ref HEAD)" \
                                       --dart-define="ESPLORA_ENDPOINT={{public_regtest_esplora}}" --dart-define="COORDINATOR_P2P_ENDPOINT={{public_regtest_coordinator}}" \
                                       --dart-define="COORDINATOR_PORT_HTTP={{public_coordinator_http_port}}" --flavor demo

release-apk-regtest: gen android-release build-apk-regtest

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

# vim:expandtab:sw=4:ts=4
