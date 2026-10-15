#!/bin/sh
set -eu

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/compiler/build"
SEED=""
UPDATE=0
CC="${BURN_CC:-${CC:-cc}}"
PLATFORMS="linux-x86_64 linux-aarch64 macos-x86_64 macos-aarch64"

usage() {
    cat <<EOF2
Builds the Burn compiler with itself, starting from the seed in the repository, and checks
that the result is stable:

  stage 0  the C compiler assembles the seed (compiler/boot/<platform>.tar.xz), the compiler
           and its runtime as assembly for this platform
  stage 1  stage 0 builds compiler/src/main.bn and lib/rt, which the C compiler assembles
  stage 2  stage 1 builds them again
  stage 3  stage 2 builds them again, unless stage 1 and stage 2 are already the same
  the last two stages must be the same assembly, byte for byte

Only a C compiler (cc), tar and xz are needed: no Burn, no Rust. Each stage is a native
executable, \$OUT/stage<n>, and \$OUT/stage<n>.s and \$OUT/runtime<n>.s are its assembly.

The seed is the compiler from an earlier commit. compiler/src may only use language features
that it understands; --update replaces the seeds of every platform with the compiler just
built, after which the sources can use everything that compiler knows.

Usage:
  scripts/bootstrap.sh [--seed <file.tar.xz>] [--out <dir>] [--update]

Options:
  --seed <file>     the seed to start from (default: compiler/boot/<this platform>.tar.xz)
  --out <dir>       where to write the stages (default: compiler/build)
  --update          write the final stage as the seeds of all platforms ($PLATFORMS)

Environment:
  BURN_CC, CC       the C compiler that assembles and links the stages (default: cc)
EOF2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --seed) SEED="$2"; shift 2 ;;
        --out) OUT="$2"; shift 2 ;;
        --update) UPDATE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "error: unknown option $1" >&2; usage >&2; exit 2 ;;
    esac
done

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) PLATFORM=linux-x86_64 ;;
    Linux-aarch64 | Linux-arm64) PLATFORM=linux-aarch64 ;;
    Darwin-x86_64) PLATFORM=macos-x86_64 ;;
    Darwin-arm64 | Darwin-aarch64) PLATFORM=macos-aarch64 ;;
    *)
        echo "error: Burn runs on x86_64 and arm64 macOS and Linux, not $(uname -s)-$(uname -m)" >&2
        exit 1
        ;;
esac
if [ -z "$SEED" ]; then
    SEED="$ROOT/compiler/boot/$PLATFORM.tar.xz"
fi

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
cd "$ROOT"

link() {
    "$CC" "$1" "$2" -lpthread -ldl -lm -o "$3"
}

stage() {
    "$OUT/stage$2" build --target runtime lib/rt/runtime.bn -o "$OUT/runtime$1.s" >/dev/null
    "$OUT/stage$2" build --target asm compiler/src/main.bn -o "$OUT/stage$1.s" >/dev/null
    link "$OUT/stage$1.s" "$OUT/runtime$1.s" "$OUT/stage$1"
}

same() {
    cmp -s "$OUT/stage$1.s" "$OUT/stage$2.s" && cmp -s "$OUT/runtime$1.s" "$OUT/runtime$2.s"
}

echo "stage 0: the C compiler assembles the seed ($(basename "$SEED"))"
rm -rf "$OUT/seed"
mkdir -p "$OUT/seed"
xz -dc "$SEED" | tar -xf - -C "$OUT/seed"
link "$OUT/seed/compiler.s" "$OUT/seed/runtime.s" "$OUT/stage0"
echo "stage 1: stage 0 builds compiler/src"
stage 1 0
echo "stage 2: stage 1 builds compiler/src"
stage 2 1
FINAL=2
if same 1 2; then
    echo "stage 1 and stage 2 are identical"
else
    echo "stage 3: stage 2 builds compiler/src"
    stage 3 2
    if ! same 2 3; then
        echo "error: stage 2 and stage 3 differ" >&2
        exit 1
    fi
    echo "stage 2 and stage 3 are identical"
fi
cp -f "$OUT/stage$FINAL" "$OUT/compiler"

if [ "$UPDATE" -eq 1 ]; then
    rm -rf "$OUT/update"
    for p in $PLATFORMS; do
        mkdir -p "$OUT/update/$p"
        BURN_PLATFORM="$p" "$OUT/compiler" build --target asm compiler/src/main.bn -o "$OUT/update/$p/compiler.s" >/dev/null
        BURN_PLATFORM="$p" "$OUT/compiler" build --target runtime lib/rt/runtime.bn -o "$OUT/update/$p/runtime.s" >/dev/null
    done
    echo "compressing the seeds"
    for p in $PLATFORMS; do
        (cd "$OUT/update/$p" && tar -cf - compiler.s runtime.s) | xz -9e -T1 >"$OUT/update/$p.tar.xz" &
    done
    wait
    for p in $PLATFORMS; do
        mv -f "$OUT/update/$p.tar.xz" "$ROOT/compiler/boot/$p.tar.xz"
        echo "updated compiler/boot/$p.tar.xz"
    done
    rm -rf "$OUT/update"
fi
