#!/usr/bin/env bash
#
# Offline end-to-end test of install.sh.
#
# Serves a stub release tree from a local Python http.server and points
# install.sh at it with SKALD_RELEASE_BASE, so the download, checksum, extract
# and install path all run without touching the network.
#
# Four cases:
#   good     a complete release installs and reports the expected version
#   bad      a corrupt archive is rejected and leaves nothing behind
#   nosum    an archive with no published .sha256 is refused outright
#   sym      an archive whose `skald` is a symlink is rejected
#
# The three failure cases assert the installer fails closed, since that is the
# property the checksums exist for.
#
# Must run from the repository root, with sh, tar and a sha256 tool available.

set -euo pipefail

port=8123
base="http://127.0.0.1:$port"
work="${RUNNER_TEMP:-$(mktemp -d)}/installer"

# The asset name install.sh derives from this machine's uname.
target=$(
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Darwin) os_part=macos ;;
        *) os_part=linux ;;
    esac
    case "$arch" in
        x86_64 | amd64) arch_part=x86_64 ;;
        *) arch_part=aarch64 ;;
    esac
    printf '%s-%s' "$os_part" "$arch_part"
)

server=''
cleanup() {
    if [ -n "$server" ]; then
        kill "$server" 2>/dev/null || true
    fi
    rm -rf "$work"
}
trap cleanup EXIT

# Runs install.sh and requires it to fail, leaving no binary behind.
# $1 what the case is called, $2 the release tag, $3 the install directory.
#
# Install directories live under $work/install so they cannot collide with the
# fixtures in $work: the symlink case stages its `skald` at $work/sym, and
# reusing that name here would make the leftover-binary check find the fixture
# itself.
expect_rejected() {
    reason=$1
    release=$2
    dir=$3

    if SKALD_RELEASE_BASE="$base" SKALD_INSTALL_DIR="$work/install/$dir" \
        ./install.sh --version "$release"; then
        echo "expected the installer to reject $reason" >&2
        exit 1
    fi
    if [ -e "$work/install/$dir/skald" ]; then
        echo "the installer left a binary behind after rejecting $reason" >&2
        exit 1
    fi
    echo "ok: rejected $reason"
}

rm -rf "$work"
mkdir -p \
    "$work/dist/v9.9.9" \
    "$work/dist/v8.8.8" \
    "$work/dist/v7.7.7" \
    "$work/sym" \
    "$work/install"

# A stand-in for the real binary; it just reports a version.
printf '#!/bin/sh\necho "skald 9.9.9"\n' > "$work/skald"
chmod 755 "$work/skald"

# v9.9.9: a complete, correctly checksummed release.
tar -czf "$work/dist/v9.9.9/skald-9.9.9-$target.tar.gz" -C "$work" skald
(
    cd "$work/dist/v9.9.9"
    sha256sum "./skald-9.9.9-$target.tar.gz" > checksums.txt
    sha256sum "./skald-9.9.9-$target.tar.gz" > "./skald-9.9.9-$target.tar.gz.sha256"
)

# v8.8.8: the archive is published but its .sha256 is not.
cp "$work/dist/v9.9.9/skald-9.9.9-$target.tar.gz" \
    "$work/dist/v8.8.8/skald-8.8.8-$target.tar.gz"

# v7.7.7: the archive is published and correctly checksummed, but its `skald`
# is a symlink pointing outside the archive, which must not be copied into
# place.
ln -s /etc/passwd "$work/sym/skald"
tar -czf "$work/dist/v7.7.7/skald-7.7.7-$target.tar.gz" -C "$work/sym" skald
(
    cd "$work/dist/v7.7.7"
    sha256sum "./skald-7.7.7-$target.tar.gz" > "./skald-7.7.7-$target.tar.gz.sha256"
)

python3 -m http.server "$port" --bind 127.0.0.1 --directory "$work/dist" \
    >/dev/null 2>&1 &
server=$!

# Poll rather than sleeping a fixed second: on a loaded runner the server can
# still be binding when the first install starts.
ready=0
for ((attempt = 0; attempt < 30; attempt++)); do
    if curl -sf -o /dev/null "$base/v9.9.9/checksums.txt"; then
        ready=1
        break
    fi
    sleep 1
done
if [ "$ready" -ne 1 ]; then
    echo "the stub http server never became reachable on port $port" >&2
    exit 1
fi

SKALD_RELEASE_BASE="$base" SKALD_INSTALL_DIR="$work/install/good" \
    ./install.sh --version v9.9.9
test -x "$work/install/good/skald"
test "$("$work/install/good/skald" --version)" = "skald 9.9.9"
echo "ok: installed a complete release"

printf 'garbage' >> "$work/dist/v9.9.9/skald-9.9.9-$target.tar.gz"
expect_rejected "a corrupt archive" v9.9.9 bad
expect_rejected "a release without a checksum" v8.8.8 nosum
expect_rejected "a symlinked skald binary" v7.7.7 sym

echo "installer smoke test passed"
