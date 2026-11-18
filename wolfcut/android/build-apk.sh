#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 6 ]]; then
    echo "Usage: $0 /path/to/android.jar /path/to/build-tools /path/to/ndk /path/to/ffmpeg-build /path/to/keystore /path/to/WolfCut.apk" >&2
    echo "Set WOLFCUT_KS_PASS to the keystore password and optionally WOLFCUT_KS_ALIAS (default: androiddebugkey)." >&2
    exit 2
fi

: "${WOLFCUT_KS_PASS:?Set the keystore password}"
export WOLFCUT_KS_PASS
platform=$(realpath "$1")
tools=$(realpath "$2")
ndk=$(realpath "$3")
media=$(realpath "$4")
keystore=$(realpath "$5")
output=$(realpath -m "$6")
project=$(cd "$(dirname "$0")/.." && pwd)
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT

export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android26-clang"
export CARGO_PROFILE_RELEASE_STRIP="${CARGO_PROFILE_RELEASE_STRIP:-symbols}"
cd "$project"
cargo build --locked --release --lib --target aarch64-linux-android --target-dir "${CARGO_TARGET_DIR:-$project/target}"
native="${CARGO_TARGET_DIR:-$project/target}/aarch64-linux-android/release/libwolfcut.so"

mkdir -p "$staging"/{classes,apk/lib/arm64-v8a,apk/assets}
java -m jdk.compiler/com.sun.tools.javac.Main -source 8 -target 8 -Xlint:-options -classpath "$platform" \
    -d "$staging/classes" android/src/com/wolfcraft/wolfcut/*.java
"$tools/d8" --release --min-api 26 --lib "$platform" --output "$staging/apk" \
    "$staging/classes/com/wolfcraft/wolfcut/"*.class

install -m 644 "$native" "$media/libffmpeg.so" "$media/libffprobe.so" "$staging/apk/lib/arm64-v8a"
install -m 644 android/FFmpeg-LICENSE.txt "$staging/apk/assets/"
"$tools/aapt2" link -I "$platform" --manifest android/AndroidManifest.xml -o "$staging/base.apk"
(cd "$staging/apk" && zip -q -r "$staging/base.apk" classes.dex lib assets)
"$tools/zipalign" -P 16 -f 4 "$staging/base.apk" "$staging/aligned.apk"
mkdir -p "$(dirname "$output")"
"$tools/apksigner" sign --ks "$keystore" --ks-key-alias "${WOLFCUT_KS_ALIAS:-androiddebugkey}" \
    --ks-pass env:WOLFCUT_KS_PASS --v4-signing-enabled false --out "$output" "$staging/aligned.apk"
"$tools/apksigner" verify "$output"
