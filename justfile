# To use this file, install Just: cargo install just
line_length := "100"

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

run:
    cd mobile && flutter run

clean:
    #!/usr/bin/env bash
    set -euxo pipefail
    cd mobile
    flutter clean
    cd native && cargo clean

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
coordinator:
    cd coordinator && cargo run

flutter-test:
    cd mobile && flutter pub run build_runner build && flutter test

native-test:
    cd mobile/native

test: flutter-test native-test

# vim:expandtab:sw=4:ts=4
