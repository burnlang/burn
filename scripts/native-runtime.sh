#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat <<EOF2
Builds the runtime that native Burn executables link against:

  libburn_runtime.a   the runtime written in Burn (lib/rt), for programs built with --target native
  libburn_bvm.a       bvm and its runtime, for native programs that run bytecode (bytecode
                      libraries and executables made from .bvm and .bar files)
  libburn_core.a      the runtime without the standard library, for --no-std programs
  native-libs.txt     the system libraries libburn_runtime.a needs, passed to the linker
  bvm-libs.txt        the system libraries libburn_bvm.a needs

A toolchain keeps them in share/burn/lib, where \`burn build --target native\` looks for them.

Usage:
  scripts/native-runtime.sh <dir>

Environment:
  BURN     the burn command that compiles lib/rt (default: bin/burn of the toolchain that
           <dir> is in, then burn)
  BURN_CC  the C compiler that assembles it (default: $CC, then cc)
  AR       the archiver (default: ar)
  RUSTC    the Rust compiler (default: rustc)
  TARGET   the Rust target triple (default: the host), such as aarch64-apple-darwin
           or aarch64-unknown-linux-gnu to build the runtime for another platform
  STRIP    the strip program (default: strip; it is skipped when it cannot read the libraries)
EOF2
}

case "${1:-}" in
    -h|--help) usage; exit 0 ;;
    "") usage >&2; exit 2 ;;
esac

OUT="$1"
RUSTC="${RUSTC:-rustc}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
if [ -z "${BURN:-}" ]; then
    BURN="$OUT/../../../bin/burn"
    [ -x "$BURN" ] || BURN=burn
fi
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
set -- --edition 2021 -C opt-level=3 -C panic=abort -C debuginfo=0 -C codegen-units=1 -C strip=debuginfo
if [ -n "${TARGET:-}" ]; then
    set -- "$@" --target "$TARGET"
fi

strip_lib() {
    case "$(uname -s)" in
        Darwin) "${STRIP:-strip}" -S "$1" 2>/dev/null || true ;;
        *) "${STRIP:-strip}" --strip-debug "$1" 2>/dev/null || true ;;
    esac
}

case "${TARGET:-}" in
    "") PLATFORM="" ;;
    x86_64-unknown-linux-*) PLATFORM=linux-x86_64 ;;
    aarch64-unknown-linux-*) PLATFORM=linux-aarch64 ;;
    x86_64-apple-darwin) PLATFORM=macos-x86_64 ;;
    aarch64-apple-darwin) PLATFORM=macos-aarch64 ;;
    *) echo "native-runtime.sh: no native code for $TARGET" >&2; exit 1 ;;
esac
if [ -n "$PLATFORM" ]; then
    BURN_PLATFORM="$PLATFORM" "$BURN" build --target runtime "$ROOT/lib/rt/runtime.bn" -o "$WORK/runtime.s" >/dev/null
else
    "$BURN" build --target runtime "$ROOT/lib/rt/runtime.bn" -o "$WORK/runtime.s" >/dev/null
fi
"${BURN_CC:-${CC:-cc}}" -c "$WORK/runtime.s" -o "$WORK/runtime.o"
rm -f "$OUT/libburn_runtime.a"
"${AR:-ar}" rcs "$OUT/libburn_runtime.a" "$WORK/runtime.o"
echo "-lpthread -ldl -lm" >"$OUT/native-libs.txt"

"$RUSTC" --crate-name bvm_runtime --crate-type rlib "$@" -o "$WORK/libbvm_runtime.rlib" "$ROOT/bvm/runtime/src/lib.rs"
CARGO_PKG_VERSION="$VERSION" "$RUSTC" --crate-name bvm --crate-type staticlib "$@" \
    --extern "bvm_runtime=$WORK/libbvm_runtime.rlib" -L "$WORK" --print native-static-libs \
    -o "$OUT/libburn_bvm.a" "$ROOT/bvm/src/lib.rs" 2>"$WORK/rustc.log" || { cat "$WORK/rustc.log" >&2; exit 1; }
sed -n 's/.*native-static-libs: *//p' "$WORK/rustc.log" | head -n 1 >"$OUT/bvm-libs.txt"
strip_lib "$OUT/libburn_bvm.a"
"$RUSTC" --crate-name bvm_runtime --crate-type staticlib --cfg burn_core "$@" -o "$OUT/libburn_core.a" "$ROOT/bvm/runtime/src/lib.rs"
strip_lib "$OUT/libburn_core.a"
