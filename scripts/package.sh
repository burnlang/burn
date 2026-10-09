#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="$ROOT/dist/burn"
BUILD=1
QUIET=0

usage() {
    cat <<EOF2
Builds the Burn toolchain and lays it out in a directory:

  <prefix>/bin/burn, burni, burnc, burn-lsp, burnfmt, bvm
  <prefix>/share/burn/compiler.bvm, tools, examples, LICENSE

compiler.bvm is the compiler written in Burn, built by itself (scripts/bootstrap.sh);
\`burn --compiler burn\` uses it.

This is what release archives contain. To install Burn, use burnup:
  curl -fsSL https://raw.githubusercontent.com/burnlang/burnup/master/install.sh | sh

Usage:
  scripts/package.sh [--prefix <dir>] [--no-build] [-q]

Options:
  --prefix <dir>   where to put the toolchain (default: dist/burn)
  --no-build       use the binaries already in target/release
  -q, --quiet      only print errors
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
    say "==> Building the compiler"
    if [ "$QUIET" -eq 1 ]; then
        (cd "$ROOT" && cargo build --release --locked --quiet)
    else
        (cd "$ROOT" && cargo build --release --locked)
    fi
fi

mkdir -p "$BIN" "$SHARE"
for exe in burn bvm; do
    cp -f "$ROOT/target/release/$exe" "$BIN/$exe.new"
    mv -f "$BIN/$exe.new" "$BIN/$exe"
done
for tool in burni burnc burn-lsp; do
    rm -f "$BIN/$tool"
    ln -s burn "$BIN/$tool" 2>/dev/null || cp -f "$BIN/burn" "$BIN/$tool"
done

rm -rf "$SHARE/tools" "$SHARE/examples"
cp -R "$ROOT/lib/tools" "$SHARE/tools"
cp -R "$ROOT/examples" "$SHARE/examples"
cp -f "$ROOT/LICENSE" "$SHARE/LICENSE"

say "==> Compiling burnfmt (the formatter is written in Burn)"
src="$SHARE/tools/fmt.bn"
rm -f "$BIN/burnfmt"
log="$(mktemp)"
if ! "$BIN/burnc" "$src" -o "$BIN/burnfmt" >/dev/null 2>"$log"; then
    printf 'warning: native compilation is not available here (%s); burnfmt will run on the interpreter\n' "$(head -n 1 "$log")" >&2
    cat >"$BIN/burnfmt" <<EOF2
#!/bin/sh
exec "$BIN/burni" "$src" "\$@"
EOF2
    chmod +x "$BIN/burnfmt"
fi
rm -f "$log"

say "==> Building the compiler written in Burn with itself"
boot="$(mktemp -d)"
if [ "$QUIET" -eq 1 ]; then
    sh "$ROOT/scripts/bootstrap.sh" --burn "$BIN/burn" --out "$boot" >/dev/null
else
    sh "$ROOT/scripts/bootstrap.sh" --burn "$BIN/burn" --out "$boot"
fi
cp -f "$boot/stage2.bvm" "$SHARE/compiler.bvm"
rm -rf "$boot"

say "==> Burn $("$BIN/burn" version | sed 's/^Burn //') is in $PREFIX"
