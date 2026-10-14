#!/bin/sh
# Copy the crate to the jumpbox (~/ctf-run-v2), build it there in release mode with https support
# (--features tls: the proxy and the external API both work), and run the tests.
# Usage: tools/deploy.sh [--no-test]
set -eu
cd "$(dirname "$0")/.."
KEY=../huntress-id_ed25519
HOST=HuntressCTF@10.1.181.90
TEST=1
[ "${1:-}" = "--no-test" ] && TEST=0
tar czf - Cargo.toml Cargo.lock src testdata tools |
  ssh -i "$KEY" -o BatchMode=yes "$HOST" "mkdir -p ~/ctf-run-v2 && cd ~/ctf-run-v2 && rm -rf src tools && tar xzf - &&
    ~/.cargo/bin/cargo build --release --features tls 2>&1 | grep -E '^(error|warning: unused)|Finished' ;
    if [ $TEST = 1 ]; then ~/.cargo/bin/cargo test --release --features tls 2>&1 | grep -E 'test result|FAILED|panicked'; fi"
