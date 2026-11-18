#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "Usage: $0 /path/to/ffmpeg-source /path/to/android-ndk /path/to/output" >&2
    exit 2
fi

source_dir=$(realpath "$1")
toolchain=$(realpath "$2")/toolchains/llvm/prebuilt/linux-x86_64/bin
mkdir -p "$3"
output=$(realpath "$3")
mkdir -p "$output/build"
cd "$output/build"

"$source_dir/configure" \
    --target-os=android --arch=aarch64 --enable-cross-compile --disable-autodetect \
    --cc="$toolchain/aarch64-linux-android26-clang" --ar="$toolchain/llvm-ar" \
    --ranlib="$toolchain/llvm-ranlib" --nm="$toolchain/llvm-nm" --strip="$toolchain/llvm-strip" \
    --extra-ldflags="-Wl,-z,max-page-size=16384" \
    --disable-doc --disable-network --disable-debug --disable-ffplay --disable-everything \
    --enable-zlib --enable-swscale --enable-swresample --enable-avfilter \
    --enable-protocol=file,pipe \
    --enable-indev=lavfi --enable-demuxer=mov,matroska,avi,mpegts,concat \
    --enable-muxer=mp4,image2pipe \
    --enable-parser=h264,hevc,av1,vp8,vp9,mpeg4video,mpegvideo,aac,opus,mpegaudio \
    --enable-decoder=h264,hevc,av1,vp8,vp9,mpeg4,mpeg2video,mjpeg,aac,aac_latm,opus,vorbis,mp3,ac3,flac,pcm_s16le,pcm_u8,wrapped_avframe \
    --enable-encoder=mpeg4,aac,png \
    --enable-filter=scale,pad,setsar,fps,apad,anullsrc,aresample,format,aformat,transpose,hflip,vflip,rotate
make -j"${JOBS:-2}" ffmpeg ffprobe
install -m 755 ffmpeg "$output/libffmpeg.so"
install -m 755 ffprobe "$output/libffprobe.so"
