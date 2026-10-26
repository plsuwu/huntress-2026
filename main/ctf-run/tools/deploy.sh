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
  # Handshakes to the box can take ~40 s, hence the long ConnectTimeout. The binary goes up as ctf-run.new and is
  # renamed over the old one only after it runs, so a running campaign never sees a missing binary; the previous
  # one stays as ctf-run.prev.
  SSH="-i $KEY -o BatchMode=yes -o ConnectTimeout=250"
  tar czf - tools | ssh $SSH "$HOST" "mkdir -p ~/ctf-run-v2/target/release && cd ~/ctf-run-v2 && rm -rf tools && tar xzf -"
  scp -q $SSH ../result/bin/ctf-run "$HOST:ctf-run-v2/target/release/ctf-run.new"
  ssh $SSH "$HOST" "cd ~/ctf-run-v2/target/release && chmod 755 ctf-run.new && ./ctf-run.new lanes >/dev/null && { [ ! -f ctf-run ] || cp -f ctf-run ctf-run.prev; } && mv -f ctf-run.new ctf-run && echo deployed static binary \$(sha256sum ctf-run | cut -c1-8)"
  exit 0
fi
shift
TEST=1
[ "${1:-}" = "--no-test" ] && TEST=0
tar czf - Cargo.toml Cargo.lock src testdata tools |
  ssh -i "$KEY" -o BatchMode=yes "$HOST" "mkdir -p ~/ctf-run-v2 && cd ~/ctf-run-v2 && rm -rf src tools && tar xzf - &&
    ~/.cargo/bin/cargo build --release --features tls 2>&1 | grep -E '^(error|warning: unused)|Finished' ;
    if [ $TEST = 1 ]; then ~/.cargo/bin/cargo test --release --features tls 2>&1 | grep -E 'test result|FAILED|panicked'; fi"
