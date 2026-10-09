#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="$ROOT/dist/burn"
STAGE0="${BURN_STAGE0:-burn}"
BUILD=1
QUIET=0

usage() {
    cat <<EOF2
Builds the Burn toolchain and lays it out in a directory:

  <prefix>/bin/burn, burni, burnc, burn-lsp, burnfmt, bvm
  <prefix>/share/burn/compiler.bvm, burn.bvm, tools, examples, LICENSE

bin/burn is bvm, the Burn virtual machine. Started as burn, burni, burnc or burn-lsp,
it runs share/burn/burn.bvm, the command line written in Burn. compiler.bvm is the
compiler written in Burn, built by itself (scripts/bootstrap.sh) starting from a
released Burn (stage0), and it builds burn.bvm.

This is what release archives contain. To install Burn, use burnup:
  curl -fsSL https://raw.githubusercontent.com/burnlang/burnup/master/install.sh | sh

Usage:
  scripts/package.sh [--prefix <dir>] [--stage0 <burn>] [--no-build] [-q]

Options:
  --prefix <dir>    where to put the toolchain (default: dist/burn)
  --stage0 <burn>   the released burn that starts the bootstrap (default: \$BURN_STAGE0 or burn on PATH)
  --no-build        use the bvm already in target/release
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
        --stage0)
            [ $# -ge 2 ] || { echo "error: --stage0 needs a burn executable" >&2; exit 2; }
            STAGE0="$2"
            shift 2
            ;;
        --no-build)
            BUILD=0
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

if [ "$BUILD" -eq 1 ]; then
    say "==> Building bvm"
    if [ "$QUIET" -eq 1 ]; then
        (cd "$ROOT" && cargo build --release --locked --quiet -p bvm)
    else
        (cd "$ROOT" && cargo build --release --locked -p bvm)
    fi
fi

say "==> Building the compiler written in Burn with itself, starting from $STAGE0"
boot="$(mktemp -d)"
if [ "$QUIET" -eq 1 ]; then
    sh "$ROOT/scripts/bootstrap.sh" --burn "$STAGE0" --out "$boot" >/dev/null
else
    sh "$ROOT/scripts/bootstrap.sh" --burn "$STAGE0" --out "$boot"
fi

mkdir -p "$BIN" "$SHARE"
for exe in burn bvm; do
    cp -f "$ROOT/target/release/bvm" "$BIN/$exe.new"
    mv -f "$BIN/$exe.new" "$BIN/$exe"
done
for tool in burni burnc burn-lsp; do
    rm -f "$BIN/$tool"
    ln -s burn "$BIN/$tool" 2>/dev/null || cp -f "$BIN/burn" "$BIN/$tool"
done
cp -f "$boot/stage2.bvm" "$SHARE/compiler.bvm"
rm -rf "$boot"

say "==> Building the command line written in Burn"
cli="$(mktemp)"
(cd "$ROOT" && "$BIN/bvm" "$SHARE/compiler.bvm" build compiler/src/bin/burn.bn -o "$cli.bvm" >/dev/null)
"$BIN/bvm" asm "$cli.bvm" -o "$SHARE/burn.bvm" >/dev/null
rm -f "$cli" "$cli.bvm"

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
