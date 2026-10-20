#!/usr/bin/env bash
# End-to-end test of cookie-feature-gate.
# Usage: tests/e2e/run-e2e.sh <cookie|stock>
#   cookie: uses the Cookie test validator built from this repository
#           (cargo build -p agave-validator --bin solana-test-validator)
#   stock:  uses solana-test-validator from PATH (stock Agave 4.1.2)
set -u
MODE="${1:?usage: run-e2e.sh <cookie|stock>}"
HERE="$(cd "$(dirname "$0")" && pwd)"
PROGRAM_DIR="$(cd "$HERE/../.." && pwd)"
REPO="$(cd "$PROGRAM_DIR/../.." && pwd)"
SO="$PROGRAM_DIR/target/deploy/cookie_feature_gate.so"
PROGRAM=CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4
F=B1ockRevenueSharing111111111111111111111111
G=CustomCommissionCo11ector111111111111111111
RPC=http://127.0.0.1:8899

if [ "$MODE" = cookie ]; then TV="$REPO/target/debug/solana-test-validator"; else TV="$(command -v solana-test-validator)"; fi
[ -x "$TV" ] || { echo "test validator not found: $TV"; exit 2; }
[ -f "$SO" ] || (cd "$PROGRAM_DIR" && cargo-build-sbf) || exit 2
[ -d "$HERE/node_modules/@solana/web3.js" ] || (cd "$HERE" && npm install --silent) || exit 2

WORK="$(mktemp -d)"
UA="$WORK/upgrade-authority.json"
solana-keygen new --no-bip39-passphrase --silent -o "$UA" >/dev/null

"$TV" --reset --quiet --ledger "$WORK/ledger" --slots-per-epoch 64 \
  --deactivate-feature "$F" --deactivate-feature "$G" \
  --upgradeable-program "$PROGRAM" "$SO" "$(solana-keygen pubkey "$UA")" > "$WORK/validator.out" 2>&1 &
TVPID=$!
trap 'kill $TVPID 2>/dev/null; wait $TVPID 2>/dev/null; rm -rf "$WORK"' EXIT
for _ in $(seq 1 120); do solana -u "$RPC" cluster-version >/dev/null 2>&1 && break; sleep 1; done
echo "validator: $("$TV" --version 2>/dev/null | head -1)"

cd "$HERE" && node --experimental-require-module --no-warnings e2e.js "$UA" "$F" "$G" "$MODE"
