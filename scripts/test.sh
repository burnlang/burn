#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TOOLCHAIN=""
JOBS=""

usage() {
    cat <<EOF2
Runs the end-to-end tests (tests/run.bn) against a toolchain: compiling and running the programs
in tests/ on bvm, natively and as JavaScript, the command line, projects and the language server.

Usage:
  scripts/test.sh [--toolchain <dir>] [-j <jobs>] [<test>...]

Options:
  --toolchain <dir>  the toolchain to test (default: one built from this checkout with
                     scripts/package.sh)
  -j <jobs>          how many tests run at once (default: the number of processors)
  <test>             only run these tests (\`burn tests/run.bn --list\` names them)
EOF2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --toolchain) TOOLCHAIN="$2"; shift 2 ;;
        -j) JOBS="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        -*) echo "error: unknown option $1" >&2; usage >&2; exit 2 ;;
        *) break ;;
    esac
done

if [ -z "$JOBS" ]; then
    JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

if [ -z "$TOOLCHAIN" ]; then
    echo "==> Building the toolchain"
    sh "$ROOT/scripts/package.sh" --prefix "$WORK/burn" -q
    TOOLCHAIN="$WORK/burn"
fi
TOOLCHAIN="$(cd "$TOOLCHAIN" && pwd)"

cd "$ROOT"
"$TOOLCHAIN/bin/burn" build tests/run.bn -o "$WORK/run" >/dev/null
if [ $# -gt 0 ]; then
    printf '%s\n' "$@" >"$WORK/tests"
else
    "$WORK/run" --list >"$WORK/tests"
fi
mkdir -p "$WORK/logs"
echo "==> Running $(wc -l <"$WORK/tests" | tr -d ' ') tests, $JOBS at a time"
start="$(date +%s)"
xargs -P "$JOBS" -I {} sh -c 'BURN_TEST_TOOLCHAIN="$1" "$2" "$3" >"$4/$3.log" 2>&1 || true; head -n 1 "$4/$3.log"' sh "$TOOLCHAIN" "$WORK/run" {} "$WORK/logs" <"$WORK/tests"
failed=0
while read -r name; do
    if ! grep -q "^test $name \.\.\. ok$" "$WORK/logs/$name.log"; then
        failed=$((failed + 1))
        echo
        echo "---- $name ----"
        cat "$WORK/logs/$name.log"
    fi
done <"$WORK/tests"
total="$(wc -l <"$WORK/tests" | tr -d ' ')"
echo
echo "$((total - failed)) passed, $failed failed in $(($(date +%s) - start))s"
[ "$failed" -eq 0 ]
