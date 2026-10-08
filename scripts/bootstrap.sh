#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BURN="${BURN:-burn}"
OUT="$ROOT/compiler/build"

usage() {
    cat <<EOF2
Builds the Burn compiler with itself and checks that the result is stable:

  stage 1  the compiler in compiler/src, run by <burn>, builds itself
  stage 2  stage 1 builds the compiler again
  stage 1 and stage 2 must be the same bvm module, byte for byte

The modules are bvm assembly that \`burn <file.bvm>\` runs.

Usage:
  scripts/bootstrap.sh [--burn <burn>] [--out <dir>]

Options:
  --burn <burn>   the burn that runs stage 1 (default: \$BURN or burn on PATH)
  --out <dir>     where to write stage1.bvm and stage2.bvm (default: compiler/build)
EOF2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --burn) BURN="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown option $1" >&2; usage >&2; exit 2 ;;
    esac
done

mkdir -p "$OUT"
cd "$ROOT"
echo "stage 1: $BURN compiler/src/main.bn"
"$BURN" compiler/src/main.bn build compiler/src/main.bn -o "$OUT/stage1.bvm"
echo "stage 2: $BURN $OUT/stage1.bvm"
"$BURN" "$OUT/stage1.bvm" build compiler/src/main.bn -o "$OUT/stage2.bvm"
if ! cmp -s "$OUT/stage1.bvm" "$OUT/stage2.bvm"; then
    echo "error: stage 1 and stage 2 differ" >&2
    exit 1
fi
echo "stage 1 and stage 2 are identical"
