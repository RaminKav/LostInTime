#!/usr/bin/env bash

# crash if it fails lol
set -euo pipefail

APP_NAME="Lost In Time"
OUT_FOLDER="build"
RUST_CRATE_NAME="survival-rogue-like" # from ur cargo toml


# get output path
APP_CONTENTS_PATH="${OUT_FOLDER}/${APP_NAME}.app/Contents"

# create .app (it's just a folder lol)
mkdir -p "${APP_CONTENTS_PATH}/Resources" # mac support files here
mkdir -p "${APP_CONTENTS_PATH}/MacOS" # binary + assets in here
# copy Info.plist that has nice 
cp Info.plist "${APP_CONTENTS_PATH}/Info.plist"
# copy the icon
cp AppIcon.icns "${APP_CONTENTS_PATH}/Resources/AppIcon.icns"
# copy game assets - normally for a mac app these go in /resources but bevy is weird and macos doesnt care lol
cp -a assets "${APP_CONTENTS_PATH}/MacOS/"

# build the binary
cargo build --release --features release-bundle --target x86_64-apple-darwin # build for Intel
cargo build --release --features release-bundle --target aarch64-apple-darwin # build for Apple Silicon
# combine the executables into a single file and put it in the right place for macos
lipo "target/x86_64-apple-darwin/release/${RUST_CRATE_NAME}" \
     "target/aarch64-apple-darwin/release/${RUST_CRATE_NAME}" \
     -create -output "${APP_CONTENTS_PATH}/MacOS/Lost In Time"
