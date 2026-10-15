#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat <<EOF2
Builds the runtime that native Burn executables link against:

  libburn_runtime.a   the runtime written in Burn (lib/rt), for programs built with --target native
  libburn_launcher.a  bvm written in Burn, for executables that run bytecode (made from .bvm and
                      .bar files, and programs that import bytecode libraries)
  libburn_core.a      the runtime without the standard library, for --no-std programs
  native-libs.txt     the system libraries libburn_runtime.a needs, passed to the linker

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

build() {
    if [ -n "$PLATFORM" ]; then
        BURN_PLATFORM="$PLATFORM" "$BURN" build "$@" >/dev/null
    else
        "$BURN" build "$@" >/dev/null
    fi
}

archive() {
    "${BURN_CC:-${CC:-cc}}" -c "$WORK/$1.s" -o "$WORK/$1.o"
    rm -f "$OUT/$2"
    "${AR:-ar}" rcs "$OUT/$2" "$WORK/$1.o"
}

build --target runtime "$ROOT/lib/rt/runtime.bn" -o "$WORK/runtime.s"
archive runtime libburn_runtime.a
build --target asm "$ROOT/compiler/src/bin/launcher.bn" -o "$WORK/launcher.s"
archive launcher libburn_launcher.a
echo "-lpthread -ldl -lm" >"$OUT/native-libs.txt"
rm -f "$OUT/libburn_bvm.a" "$OUT/bvm-libs.txt"

"$RUSTC" --crate-name bvm_runtime --crate-type staticlib --cfg burn_core "$@" -o "$OUT/libburn_core.a" "$ROOT/bvm/runtime/src/lib.rs"
strip_lib "$OUT/libburn_core.a"
