#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PREFIX="$ROOT/dist/burn-windows"
QUIET=0

usage() {
    cat <<EOF2
Builds the Burn toolchain for Windows x86-64 on Linux or macOS, with the toolchain of this
machine and a mingw-w64 C compiler:

  <prefix>/bin/burn.exe, burni.exe, burnc.exe, burn-lsp.exe, bvm.exe, burnfmt.cmd
  <prefix>/share/burn/lib, tools, examples, LICENSE

The layout is the one scripts/package.sh makes, so the folder can be zipped and unpacked
anywhere on Windows; burn.exe finds share/burn next to its bin folder.

Usage:
  scripts/package-windows.sh [--prefix <dir>] [-q]

Options:
  --prefix <dir>    where to put the toolchain (default: dist/burn-windows)
  -q, --quiet       only print errors

Environment:
  BURN      the burn command of this machine (default: bin/burn of dist/burn, then burn)
  MINGW_CC  the mingw-w64 C compiler (default: x86_64-w64-mingw32-gcc)
  MINGW_AR  its archiver (default: x86_64-w64-mingw32-ar)
EOF2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || { echo "error: --prefix needs a directory" >&2; exit 2; }
            PREFIX="$2"
            shift 2
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

say() {
    if [ "$QUIET" -eq 0 ]; then
        printf '%s\n' "$*"
    fi
}

if [ -z "${BURN:-}" ]; then
    BURN="$ROOT/dist/burn/bin/burn"
    [ -x "$BURN" ] || BURN=burn
fi
CC_W="${MINGW_CC:-x86_64-w64-mingw32-gcc}"
AR_W="${MINGW_AR:-x86_64-w64-mingw32-ar}"
command -v "$CC_W" >/dev/null 2>&1 || { echo "error: $CC_W not found; install mingw-w64 or set MINGW_CC" >&2; exit 1; }

BIN="$PREFIX/bin"
SHARE="$PREFIX/share/burn"
rm -rf "$PREFIX"
mkdir -p "$BIN" "$SHARE"

say "==> Building the runtime for Windows executables"
BURN="$BURN" BURN_CC="$CC_W" AR="$AR_W" TARGET=x86_64-w64-mingw32 sh "$ROOT/scripts/native-runtime.sh" "$SHARE/lib"

say "==> Building burn.exe"
(cd "$ROOT" && BURN_PLATFORM=windows-x86_64 BURN_CC="$CC_W" BURN_RUNTIME_DIR="$SHARE/lib" "$BURN" build compiler/src/bin/burn.bn -o "$BIN/burn" >/dev/null)
for tool in burni burnc burn-lsp bvm; do
    cp -f "$BIN/burn.exe" "$BIN/$tool.exe"
done
printf '@"%%~dp0burni.exe" "%%~dp0..\\share\\burn\\tools\\fmt.bn" %%*\r\n' >"$BIN/burnfmt.cmd"

cp -R "$ROOT/lib/tools" "$SHARE/tools"
cp -R "$ROOT/examples" "$SHARE/examples"
cp -f "$ROOT/LICENSE" "$SHARE/LICENSE"

say "==> Burn for Windows is in $PREFIX"
