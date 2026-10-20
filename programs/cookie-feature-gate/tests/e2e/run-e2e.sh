#!/usr/bin/env bash
# End-to-end test of the Cookie feature switches.
# Usage: tests/e2e/run-e2e.sh <cookie|stock>
#   cookie: uses the Cookie binaries built from this repository:
#           cargo build -p agave-validator --bin solana-test-validator -p solana-cli --bin solana
#   stock:  uses solana-test-validator and solana from PATH (stock Agave 4.1.2)
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
RPC2=http://127.0.0.1:9799

if [ "$MODE" = cookie ]; then
  TV="$REPO/target/debug/solana-test-validator"; CLI="$REPO/target/debug/solana"
else
  TV="$(command -v solana-test-validator)"; CLI="$(command -v solana)"
fi
STOCK_CLI="$(command -v solana)"
[ -x "$TV" ] && [ -x "$CLI" ] || { echo "binaries not found: $TV $CLI"; exit 2; }
[ -f "$SO" ] || (cd "$PROGRAM_DIR" && cargo-build-sbf) || exit 2
[ -d "$HERE/node_modules/@solana/web3.js" ] || (cd "$HERE" && npm install --silent) || exit 2

WORK="$(mktemp -d)"
UA="$WORK/upgrade-authority.json"
solana-keygen new --no-bip39-passphrase --silent -o "$UA" >/dev/null
PIDS=()
trap 'kill "${PIDS[@]}" 2>/dev/null; wait 2>/dev/null; rm -rf "$WORK"' EXIT
wait_rpc() { for _ in $(seq 1 180); do "$STOCK_CLI" -u "$1" cluster-version >/dev/null 2>&1 && return 0; sleep 1; done; return 1; }

"$TV" --reset --quiet --ledger "$WORK/ledger" --slots-per-epoch 64 \
  --deactivate-feature "$F" --deactivate-feature "$G" \
  --upgradeable-program "$PROGRAM" "$SO" "$(solana-keygen pubkey "$UA")" > "$WORK/validator.out" 2>&1 &
PIDS+=($!)
wait_rpc "$RPC" || { echo "validator did not start"; exit 2; }
echo "validator: $("$TV" --version 2>/dev/null | head -1)"

cd "$HERE" && node --experimental-require-module --no-warnings e2e.js "$UA" "$F" "$G" "$MODE"
RC=$?

# Tool checks: the CLI must report feature status exactly as Solana would.
FAIL=0
pass() { echo "PASS  $1"; }
fail() { echo "FAIL  $1"; FAIL=1; }
status_of() { "$1" -u "$2" feature status "$3" --output json 2>/dev/null | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{const f=JSON.parse(s).features[0];const st=f.status;console.log(typeof st==="string"?st:(st.type||Object.keys(st)[0]))}catch(e){console.log("unknown")}})'; }
SF=$(status_of "$CLI" "$RPC" "$F"); SG=$(status_of "$CLI" "$RPC" "$G")
echo "solana feature status ($MODE CLI): F=$SF G=$SG"
if [ "$MODE" = cookie ]; then
  case "$SF" in active*|Active*) pass "Cookie CLI shows the alias-activated feature as active";; *) fail "Cookie CLI shows F as active (got $SF)";; esac
  case "$SG" in inactive*|Inactive*) pass "Cookie CLI shows the revoked feature as inactive";; *) fail "Cookie CLI shows G as inactive (got $SG)";; esac
  SS=$(status_of "$STOCK_CLI" "$RPC" "$F")
  echo "solana feature status (stock CLI): F=$SS"
  case "$SS" in inactive*|Inactive*) pass "stock CLI cannot see the alias (expected without the Cookie hook)";; *) fail "stock CLI unexpectedly sees F (got $SS)";; esac

  # --clone-feature-set from the Cookie cluster must carry the alias-activated feature.
  # The clone reads finalized state, so wait until the activation slot is finalized.
  for _ in $(seq 1 120); do [ "$("$STOCK_CLI" -u "$RPC" slot --commitment finalized 2>/dev/null || echo 0)" -gt 64 ] && break; sleep 1; done
  "$TV" --reset --quiet --ledger "$WORK/ledger2" --rpc-port 9799 --faucet-port 9810 \
    --gossip-port 9001 --dynamic-port-range 9002-9040 \
    --clone-feature-set --url "$RPC" > "$WORK/validator2.out" 2>&1 &
  PIDS+=($!)
  if wait_rpc "$RPC2"; then
    SC=$(status_of "$CLI" "$RPC2" "$F")
    case "$SC" in active*|Active*) pass "test validator cloned with --clone-feature-set has F active";; *) fail "cloned test validator has F active (got $SC)";; esac
  else
    fail "cloned test validator did not start"; tail -5 "$WORK/validator2.out"
  fi
fi

[ $RC -eq 0 ] && [ $FAIL -eq 0 ] && echo "E2E PASSED" || { echo "E2E FAILED"; exit 1; }
