#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="$ROOT/dist/burn"
QUIET=0

usage() {
    cat <<EOF2
Builds the Burn toolchain and lays it out in a directory:

  <prefix>/bin/burn, burni, burnc, burn-lsp, bvm, burnfmt
  <prefix>/share/burn/lib, tools, examples, LICENSE

bin/burn is the Burn toolchain written in Burn, a native executable: started as burn, burni,
burnc or burn-lsp it is the command line, and started as bvm it is the Burn virtual machine.
It is built by the compiler written in Burn, which builds itself first (scripts/bootstrap.sh)
starting from the seed in compiler/boot, the compiler as assembly for this platform. Only a C
compiler, tar and xz are needed. lib holds the runtime that native executables link against
(scripts/native-runtime.sh). The toolchain runs on x86_64 and arm64 (Apple silicon) macOS and
Linux.

This is what release archives contain. To install Burn, use burnup:
  curl -fsSL https://raw.githubusercontent.com/burnlang/burnup/master/install.sh | sh

Usage:
  scripts/package.sh [--prefix <dir>] [-q]

Options:
  --prefix <dir>    where to put the toolchain (default: dist/burn)
  -q, --quiet       only print errors
EOF2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || { echo "error: --prefix needs a directory" >&2; exit 2; }
            PREFIX="$2"
            shift 2
            ;;
        --prefix=*)
            PREFIX="${1#--prefix=}"
            shift
            ;;
        -q | --quiet)
            QUIET=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown option: $1 (see --help)" >&2
            exit 2
            ;;
    esac
done

case "$PREFIX" in
    /*) ;;
    *) PREFIX="$(pwd)/$PREFIX" ;;
esac
BIN="$PREFIX/bin"
SHARE="$PREFIX/share/burn"

say() {
    if [ "$QUIET" -eq 0 ]; then
        printf '%s\n' "$*"
    fi
}

say "==> Building the compiler written in Burn with itself, starting from the seed in compiler/boot"
boot="$(mktemp -d)"
if [ "$QUIET" -eq 1 ]; then
    sh "$ROOT/scripts/bootstrap.sh" --out "$boot" >/dev/null
else
    sh "$ROOT/scripts/bootstrap.sh" --out "$boot"
fi

say "==> Building the toolchain written in Burn with the compiler"
work="$(mktemp -d)"
(cd "$ROOT" && "$boot/compiler" build --target runtime lib/rt/runtime.bn -o "$work/runtime.s" >/dev/null)
(cd "$ROOT" && "$boot/compiler" build --target asm compiler/src/bin/burn.bn -o "$work/burn.s" >/dev/null)
mkdir -p "$BIN" "$SHARE"
"${BURN_CC:-${CC:-cc}}" "$work/burn.s" "$work/runtime.s" -lpthread -ldl -lm -o "$BIN/burn.new"
mv -f "$BIN/burn.new" "$BIN/burn"
rm -rf "$work" "$boot"
for tool in burni burnc burn-lsp bvm; do
    rm -f "$BIN/$tool"
    ln -s burn "$BIN/$tool" 2>/dev/null || cp -f "$BIN/burn" "$BIN/$tool"
done
rm -f "$SHARE/compiler.bvm" "$SHARE/burn.bvm"

say "==> Building the runtime for native executables"
rm -rf "$SHARE/lib"
BURN="$BIN/burn" sh "$ROOT/scripts/native-runtime.sh" "$SHARE/lib"

rm -rf "$SHARE/tools" "$SHARE/examples"
cp -R "$ROOT/lib/tools" "$SHARE/tools"
cp -R "$ROOT/examples" "$SHARE/examples"
cp -f "$ROOT/LICENSE" "$SHARE/LICENSE"

rm -f "$BIN/burnfmt"
cat >"$BIN/burnfmt" <<'EOF2'
#!/bin/sh
here="$(cd "$(dirname "$0")" && pwd)"
exec "$here/burni" "$here/../share/burn/tools/fmt.bn" "$@"
EOF2
chmod +x "$BIN/burnfmt"

say "==> Burn $("$BIN/burn" version | sed 's/^Burn //') is in $PREFIX"
