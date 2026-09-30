#!/bin/sh
set -eu

url="https://raw.githubusercontent.com/burnlang/burnup/master/install.sh"
printf '%s\n' "The Burn installer is now burnup: $url" >&2
if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" | sh -s -- "$@"
elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$url" | sh -s -- "$@"
else
    printf '%s\n' "error: curl or wget is needed to download burnup" >&2
    exit 1
fi
