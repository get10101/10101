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

alias b := build
build:
    cd rust && cargo build

alias g := gen
gen:
    flutter pub get
    flutter_rust_bridge_codegen \
    		--rust-input rust/src/api.rs \
            --rust-output rust/src/bridge_generated/bridge_generated.rs \
            --dart-output lib/bridge_generated/bridge_generated.dart \
            --dart-format-line-length {{line_length}} \
            --c-output ios/Runner/bridge_generated.h \
            --extra-c-output-path macos/Runner/bridge_generated.h \
            --dart-decl-output lib/bridge_generated/bridge_definitions.dart

alias t := test
test:
    flutter test integration_test/main.dart

alias c := clean
clean:
    flutter clean
    cd rust && cargo clean

alias l := lint
lint: lint-flutter clippy

lint-flutter:
    flutter analyze --fatal-infos .

clippy:
    cd rust && cargo clippy --all-targets -- -D warnings


## native: Build Rust library for native target (to run on your desktop)
native:
	cd rust && cargo build

# Build Rust library for Android native targets
android:
	cd rust && cargo ndk -o ../android/app/src/main/jniLibs build

# ios: Build Rust library for iOS
ios:
	cd rust && cargo lipo
	cp rust/target/universal/debug/libtentenone.a ios/Runner

run:
    flutter run

## format: Format all files in the project
alias fmt := format
format: dprint flutter-format

dprint:
	dprint fmt

# Flutter lacks a dprint plugin, use its own formatter
flutter-format:
	flutter format . --fix --line-length {{line_length}}