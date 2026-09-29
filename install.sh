#!/bin/sh
set -eu

BURN_REPO="${BURN_REPO:-https://github.com/burnlang/burn}"
BURN_REF="${BURN_REF:-master}"
PREFIX="${BURN_HOME:-$HOME/.burn}"
FROM_SOURCE=0
MODIFY_PATH=1
INSTALL_RUST=0
UNINSTALL=0
QUIET=0

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    BOLD="$(printf '\033[1m')"
    RED="$(printf '\033[31m')"
    GREEN="$(printf '\033[32m')"
    YELLOW="$(printf '\033[33m')"
    RESET="$(printf '\033[0m')"
else
    BOLD=""
    RED=""
    GREEN=""
    YELLOW=""
    RESET=""
fi

say() {
    if [ "$QUIET" -eq 0 ]; then
        printf '%s\n' "$*"
    fi
}

step() {
    say "${BOLD}==>${RESET} $*"
}

warn() {
    printf '%s\n' "${YELLOW}warning:${RESET} $*" >&2
}

die() {
    printf '%s\n' "${RED}error:${RESET} $*" >&2
    exit 1
}

usage() {
    cat <<EOF
Burn toolchain installer

Installs burn, burni (interpreter), burnc (compiler), burnfmt (formatter)
and burn-lsp (language server) into \$BURN_HOME/bin (default: ~/.burn/bin).

Usage:
  install.sh [options]

Options:
  --prefix <dir>      install into <dir> instead of ~/.burn
  --ref <git ref>     branch, tag or commit to build (default: master)
  --from-source       always build from source instead of downloading a release
  --install-rust      install Rust with rustup if cargo is missing
  --no-modify-path    do not add the bin directory to your shell profile
  --uninstall         remove an existing installation
  -q, --quiet         only print errors
  -h, --help          show this help

Environment:
  BURN_HOME           installation directory (same as --prefix)
  BURN_REPO           git repository to build from
  BURN_REF            git ref to build from (same as --ref)

Examples:
  curl -fsSL https://raw.githubusercontent.com/burnlang/burn/master/install.sh | sh
  ./install.sh --prefix /opt/burn --no-modify-path
EOF
}

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            [ $# -ge 2 ] || die "--prefix needs a directory"
            PREFIX="$2"
            shift 2
            ;;
        --prefix=*)
            PREFIX="${1#--prefix=}"
            shift
            ;;
        --ref)
            [ $# -ge 2 ] || die "--ref needs a value"
            BURN_REF="$2"
            shift 2
            ;;
        --ref=*)
            BURN_REF="${1#--ref=}"
            shift
            ;;
        --from-source)
            FROM_SOURCE=1
            shift
            ;;
        --install-rust)
            INSTALL_RUST=1
            shift
            ;;
        --no-modify-path)
            MODIFY_PATH=0
            shift
            ;;
        --uninstall)
            UNINSTALL=1
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
            die "unknown option: $1 (see --help)"
            ;;
    esac
done

case "$PREFIX" in
    /*) ;;
    *) PREFIX="$(pwd)/$PREFIX" ;;
esac
BIN="$PREFIX/bin"
SHARE="$PREFIX/share/burn"
ENV_FILE="$PREFIX/env"
MARKER="# added by the Burn installer"

need() {
    command -v "$1" >/dev/null 2>&1
}

profiles() {
    for f in "$HOME/.profile" "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.zshrc" "$HOME/.zprofile"; do
        if [ -f "$f" ]; then
            printf '%s\n' "$f"
        fi
    done
}

remove_path_lines() {
    for f in $(profiles) "$HOME/.config/fish/conf.d/burn.fish"; do
        [ -f "$f" ] || continue
        if grep -q "$MARKER" "$f" 2>/dev/null; then
            tmp="$f.burn-tmp"
            grep -v "$MARKER" "$f" >"$tmp" || true
            cat "$tmp" >"$f"
            rm -f "$tmp"
        fi
    done
    rm -f "$HOME/.config/fish/conf.d/burn.fish"
}

if [ "$UNINSTALL" -eq 1 ]; then
    if [ ! -x "$BIN/burn" ] && [ ! -d "$SHARE" ]; then
        die "no Burn installation found in $PREFIX"
    fi
    step "Removing $PREFIX"
    rm -f "$BIN/burn" "$BIN/burni" "$BIN/burnc" "$BIN/burnfmt" "$BIN/burn-lsp"
    rm -rf "$SHARE" "$ENV_FILE"
    rmdir "$BIN" 2>/dev/null || true
    rmdir "$PREFIX/share" 2>/dev/null || true
    rmdir "$PREFIX" 2>/dev/null || true
    remove_path_lines
    say "${GREEN}Burn has been uninstalled.${RESET}"
    exit 0
fi

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
    Linux) OS_NAME=linux ;;
    Darwin) OS_NAME=macos ;;
    *) OS_NAME="$(printf '%s' "$OS" | tr '[:upper:]' '[:lower:]')" ;;
esac
case "$ARCH" in
    x86_64 | amd64) ARCH_NAME=x86_64 ;;
    arm64 | aarch64) ARCH_NAME=aarch64 ;;
    *) ARCH_NAME="$ARCH" ;;
esac

WORK="$(mktemp -d 2>/dev/null || mktemp -d -t burn-install)"
cleanup() {
    rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

download() {
    if need curl; then
        curl -fsSL "$1" -o "$2"
    elif need wget; then
        wget -q "$1" -O "$2"
    else
        return 1
    fi
}

install_release() {
    asset="burn-$OS_NAME-$ARCH_NAME.tar.gz"
    url="$BURN_REPO/releases/latest/download/$asset"
    if [ -n "${BURN_RELEASE_URL:-}" ]; then
        url="$BURN_RELEASE_URL"
    fi
    step "Looking for a prebuilt release ($asset)"
    if ! download "$url" "$WORK/$asset" 2>/dev/null; then
        say "    no prebuilt release available, building from source"
        return 1
    fi
    mkdir -p "$WORK/release"
    tar -xzf "$WORK/$asset" -C "$WORK/release" || return 1
    root="$WORK/release"
    if [ -d "$WORK/release/burn" ]; then
        root="$WORK/release/burn"
    fi
    [ -x "$root/bin/burn" ] || return 1
    mkdir -p "$BIN" "$SHARE"
    cp -f "$root/bin/burn" "$BIN/burn"
    if [ -d "$root/share/burn" ]; then
        cp -R "$root/share/burn/." "$SHARE/"
    fi
    if [ -f "$root/bin/burnfmt" ] && [ ! -L "$root/bin/burnfmt" ] && [ "$(head -c 2 "$root/bin/burnfmt")" != "#!" ]; then
        cp -f "$root/bin/burnfmt" "$BIN/burnfmt"
    fi
    return 0
}

ensure_cargo() {
    if need cargo; then
        return 0
    fi
    if [ -x "$HOME/.cargo/bin/cargo" ]; then
        PATH="$HOME/.cargo/bin:$PATH"
        export PATH
        return 0
    fi
    if [ "$INSTALL_RUST" -eq 1 ]; then
        step "Installing Rust with rustup"
        download "https://sh.rustup.rs" "$WORK/rustup.sh" || die "could not download rustup"
        sh "$WORK/rustup.sh" -y --profile minimal >/dev/null || die "rustup failed"
        PATH="$HOME/.cargo/bin:$PATH"
        export PATH
        return 0
    fi
    die "cargo was not found. Install Rust from https://rustup.rs or rerun with --install-rust"
}

find_source() {
    script_dir=""
    case "$0" in
        */*) script_dir="$(cd "$(dirname "$0")" 2>/dev/null && pwd || true)" ;;
    esac
    if [ -n "$script_dir" ] && [ -f "$script_dir/Cargo.toml" ] && [ -d "$script_dir/crates/burn" ]; then
        SRC="$script_dir"
        say "    using the source tree in $SRC"
        return
    fi
    need git || die "git is required to download the Burn sources"
    step "Downloading Burn ($BURN_REF) from $BURN_REPO"
    if ! git clone --quiet --depth 1 --branch "$BURN_REF" "$BURN_REPO" "$WORK/src" 2>/dev/null; then
        git clone --quiet "$BURN_REPO" "$WORK/src" || die "could not clone $BURN_REPO"
        git -C "$WORK/src" checkout --quiet "$BURN_REF" || die "unknown ref $BURN_REF"
    fi
    SRC="$WORK/src"
}

install_source() {
    ensure_cargo
    find_source
    step "Building the compiler (this takes a minute the first time)"
    if [ "$QUIET" -eq 1 ]; then
        (cd "$SRC" && cargo build --release --locked --quiet) || die "the build failed"
    else
        (cd "$SRC" && cargo build --release --locked) || die "the build failed"
    fi
    mkdir -p "$BIN" "$SHARE"
    cp -f "$SRC/target/release/burn" "$BIN/burn.new"
    mv -f "$BIN/burn.new" "$BIN/burn"
    rm -rf "$SHARE/tools" "$SHARE/examples"
    mkdir -p "$SHARE/tools"
    cp -R "$SRC/tools/burnfmt" "$SHARE/tools/burnfmt"
    cp -R "$SRC/examples" "$SHARE/examples"
    cp -f "$SRC/install.sh" "$SHARE/install.sh"
    cp -f "$SRC/LICENSE" "$SHARE/LICENSE" 2>/dev/null || true
}

link_tool() {
    rm -f "$BIN/$1"
    if ln -s burn "$BIN/$1" 2>/dev/null; then
        return
    fi
    cp -f "$BIN/burn" "$BIN/$1"
}

build_burnfmt() {
    src="$SHARE/tools/burnfmt/burnfmt.bn"
    [ -f "$src" ] || return 0
    if [ -f "$BIN/burnfmt" ] && [ ! -L "$BIN/burnfmt" ] && [ "$BIN/burnfmt" -nt "$src" ]; then
        return 0
    fi
    step "Compiling burnfmt (the formatter is written in Burn)"
    rm -f "$BIN/burnfmt"
    if "$BIN/burnc" "$src" -o "$BIN/burnfmt" >/dev/null 2>"$WORK/burnfmt.log"; then
        return 0
    fi
    warn "native compilation is not available here ($(head -n 1 "$WORK/burnfmt.log")); burnfmt will run on the interpreter"
    cat >"$BIN/burnfmt" <<EOF
#!/bin/sh
exec "$BIN/burni" "$src" "\$@"
EOF
    chmod +x "$BIN/burnfmt"
}

write_env() {
    cat >"$ENV_FILE" <<EOF
case ":\${PATH}:" in
    *:"$BIN":*) ;;
    *) export PATH="$BIN:\$PATH" ;;
esac
EOF
}

modify_path() {
    write_env
    if [ "$MODIFY_PATH" -eq 0 ]; then
        return
    fi
    case ":$PATH:" in
        *":$BIN:"*) return ;;
    esac
    line=". \"$ENV_FILE\" $MARKER"
    updated=""
    for f in $(profiles); do
        if ! grep -q "$MARKER" "$f" 2>/dev/null; then
            printf '\n%s\n' "$line" >>"$f"
        fi
        updated="$updated $f"
    done
    if [ -z "$updated" ]; then
        printf '%s\n' "$line" >>"$HOME/.profile"
        updated=" $HOME/.profile"
    fi
    if [ -d "$HOME/.config/fish" ]; then
        mkdir -p "$HOME/.config/fish/conf.d"
        printf 'fish_add_path -g "%s" %s\n' "$BIN" "$MARKER" >"$HOME/.config/fish/conf.d/burn.fish"
        updated="$updated $HOME/.config/fish/conf.d/burn.fish"
    fi
    say "    added $BIN to PATH in:$updated"
    PATH_CHANGED=1
}

say "${BOLD}Installing the Burn toolchain into $PREFIX${RESET}"
PATH_CHANGED=0

installed=0
if [ "$FROM_SOURCE" -eq 0 ]; then
    if install_release; then
        installed=1
    fi
fi
if [ "$installed" -eq 0 ]; then
    install_source
fi

link_tool burni
link_tool burnc
link_tool burn-lsp
build_burnfmt
modify_path

"$BIN/burn" version >/dev/null 2>&1 || die "the installed burn binary does not run"
check="$WORK/hello.bn"
printf 'print("ok")\n' >"$check"
[ "$("$BIN/burni" "$check")" = "ok" ] || die "burni could not run a test program"
[ "$(printf 'var  x=1\n' | "$BIN/burnfmt")" = "var x = 1" ] || die "burnfmt did not format a test program"

say ""
say "${GREEN}Burn $("$BIN/burn" version | sed 's/^Burn //') is installed.${RESET}"
say ""
say "  burn      $BIN/burn        run, build, check, fmt, repl, lsp"
say "  burni     $BIN/burni       interpreter and REPL"
say "  burnc     $BIN/burnc       native and JavaScript compiler"
say "  burnfmt   $BIN/burnfmt     code formatter (written in Burn)"
say "  burn-lsp  $BIN/burn-lsp    language server for editors"
say ""
if [ "$PATH_CHANGED" -eq 1 ]; then
    say "Restart your shell or run:  . \"$ENV_FILE\""
elif [ "$MODIFY_PATH" -eq 0 ]; then
    case ":$PATH:" in
        *":$BIN:"*) ;;
        *) say "Add $BIN to your PATH, for example:  . \"$ENV_FILE\"" ;;
    esac
fi
say "Uninstall with:  sh \"$SHARE/install.sh\" --uninstall --prefix \"$PREFIX\""
