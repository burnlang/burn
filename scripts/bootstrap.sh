#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BVM="${BVM:-$ROOT/target/release/bvm}"
BOOT="$ROOT/compiler/boot/compiler.bvmc"
OUT="$ROOT/compiler/build"
UPDATE=0

usage() {
    cat <<EOF2
Builds the Burn compiler with itself, starting from the boot compiler in the repository,
and checks that the result is stable:

  stage 1  the boot compiler (compiler/boot/compiler.bvmc) builds compiler/src
  stage 2  stage 1 builds compiler/src again
  stage 3  stage 2 builds compiler/src again, unless stage 1 and stage 2 are already the same
  the last two stages must be the same bvm module, byte for byte

bvm runs every stage, so only Rust is needed to start: cargo builds bvm, and bvm runs the
compiler written in Burn. The modules are bvm assembly that \`bvm <file.bvm>\` runs.

The boot compiler is a compiled copy of the compiler from an earlier commit. compiler/src may
only use language features that it understands; --update replaces it with the compiler just
built, after which the sources can use everything that compiler knows.

Usage:
  scripts/bootstrap.sh [--bvm <bvm>] [--boot <compiler.bvmc>] [--out <dir>] [--update]

Options:
  --bvm <bvm>       the bvm that runs the stages (default: \$BVM or target/release/bvm, built with cargo if missing)
  --boot <file>     the compiler that starts the bootstrap (default: compiler/boot/compiler.bvmc)
  --out <dir>       where to write the stages (default: compiler/build)
  --update          write the final stage to compiler/boot/compiler.bvmc
EOF2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --bvm) BVM="$2"; shift 2 ;;
        --boot) BOOT="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --update) UPDATE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown option $1" >&2; usage >&2; exit 2 ;;
    esac
done

if [ ! -x "$BVM" ]; then
    echo "building bvm"
    (cd "$ROOT" && cargo build --release --locked --quiet -p bvm)
fi

mkdir -p "$OUT"
cd "$ROOT"
echo "stage 1: the boot compiler builds compiler/src"
"$BVM" "$BOOT" build compiler/src/main.bn -o "$OUT/stage1.bvm"
echo "stage 2: stage 1 builds compiler/src"
"$BVM" "$OUT/stage1.bvm" build compiler/src/main.bn -o "$OUT/stage2.bvm"
if cmp -s "$OUT/stage1.bvm" "$OUT/stage2.bvm"; then
    echo "stage 1 and stage 2 are identical"
else
    echo "stage 3: stage 2 builds compiler/src"
    "$BVM" "$OUT/stage2.bvm" build compiler/src/main.bn -o "$OUT/stage3.bvm"
    if ! cmp -s "$OUT/stage2.bvm" "$OUT/stage3.bvm"; then
        echo "error: stage 2 and stage 3 differ" >&2
        exit 1
    fi
    echo "stage 2 and stage 3 are identical"
fi
if [ "$UPDATE" -eq 1 ]; then
    "$BVM" asm "$OUT/stage2.bvm" -o "$ROOT/compiler/boot/compiler.bvmc" >/dev/null
    echo "updated compiler/boot/compiler.bvmc"
fi
