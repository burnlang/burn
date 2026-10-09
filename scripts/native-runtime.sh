#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

usage() {
    cat <<EOF2
Builds the runtime that native Burn executables link against:

  libburn_runtime.a   bvm and the full runtime, for programs built with --target native
  libburn_core.a      the runtime without the standard library, for --no-std programs
  native-libs.txt     the system libraries libburn_runtime.a needs, passed to the linker

A toolchain keeps them in share/burn/lib, where \`burn build --target native\` looks for them.

Usage:
  scripts/native-runtime.sh <dir>

Environment:
  RUSTC    the Rust compiler (default: rustc)
  TARGET   the Rust target triple (default: the host)
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
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
set -- --edition 2021 -C opt-level=3 -C panic=abort -C debuginfo=0 -C codegen-units=1 -C strip=debuginfo
if [ -n "${TARGET:-}" ]; then
    set -- "$@" --target "$TARGET"
fi

strip_lib() {
    case "$(uname -s)" in
        Darwin) strip -S "$1" 2>/dev/null || true ;;
        *) strip --strip-debug "$1" 2>/dev/null || true ;;
    esac
}

"$RUSTC" --crate-name bvm_runtime --crate-type rlib "$@" -o "$WORK/libbvm_runtime.rlib" "$ROOT/bvm/runtime/src/lib.rs"
CARGO_PKG_VERSION="$VERSION" "$RUSTC" --crate-name bvm --crate-type staticlib "$@" \
    --extern "bvm_runtime=$WORK/libbvm_runtime.rlib" -L "$WORK" --print native-static-libs \
    -o "$OUT/libburn_runtime.a" "$ROOT/bvm/src/lib.rs" 2>"$WORK/rustc.log" || { cat "$WORK/rustc.log" >&2; exit 1; }
sed -n 's/.*native-static-libs: *//p' "$WORK/rustc.log" | head -n 1 >"$OUT/native-libs.txt"
strip_lib "$OUT/libburn_runtime.a"
"$RUSTC" --crate-name bvm_runtime --crate-type staticlib --cfg burn_core "$@" -o "$OUT/libburn_core.a" "$ROOT/bvm/runtime/src/lib.rs"
strip_lib "$OUT/libburn_core.a"
