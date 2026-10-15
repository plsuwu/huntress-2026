#!/bin/sh
# Deploy ctf-run to the jumpbox (~/ctf-run-v2).
#
# Default: build the static musl binary locally (`nix build .#ctf-run`: https + mimalloc) and copy it
# to ~/ctf-run-v2/target/release/ctf-run with the tools - nothing compiles on the t3.small, whose
# burstable CPU credits the timed runs need. Run `cargo test` + `ctf-run replay fixtures/...` locally first.
#
# --remote-build: the old path - copy the crate and build + test it on the jumpbox (stable Rust there).
# Usage: tools/deploy.sh [--remote-build [--no-test]]
set -eu
cd "$(dirname "$0")/.."
KEY=../huntress-id_ed25519
HOST=HuntressCTF@10.1.181.90
if [ "${1:-}" != "--remote-build" ]; then
  (cd .. && nix build .#ctf-run)
  tar czf - tools | ssh -i "$KEY" -o BatchMode=yes "$HOST" "mkdir -p ~/ctf-run-v2/target/release && cd ~/ctf-run-v2 && rm -rf tools && tar xzf - && rm -f target/release/ctf-run"
  scp -q -i "$KEY" ../result/bin/ctf-run "$HOST:ctf-run-v2/target/release/ctf-run"
  ssh -i "$KEY" -o BatchMode=yes "$HOST" "chmod 755 ~/ctf-run-v2/target/release/ctf-run && ~/ctf-run-v2/target/release/ctf-run lanes >/dev/null && echo deployed static binary"
  exit 0
fi
shift
TEST=1
[ "${1:-}" = "--no-test" ] && TEST=0
tar czf - Cargo.toml Cargo.lock src testdata tools |
  ssh -i "$KEY" -o BatchMode=yes "$HOST" "mkdir -p ~/ctf-run-v2 && cd ~/ctf-run-v2 && rm -rf src tools && tar xzf - &&
    ~/.cargo/bin/cargo build --release --features tls 2>&1 | grep -E '^(error|warning: unused)|Finished' ;
    if [ $TEST = 1 ]; then ~/.cargo/bin/cargo test --release --features tls 2>&1 | grep -E 'test result|FAILED|panicked'; fi"
