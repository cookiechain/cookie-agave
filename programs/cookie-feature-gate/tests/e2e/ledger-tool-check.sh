#!/usr/bin/env bash
# Checks that agave-ledger-tool can deactivate a feature that was activated
# through a Cookie alias, as needed for an emergency restart.
# Usage: tests/e2e/ledger-tool-check.sh
# Needs the Cookie test validator (main workspace) and agave-ledger-tool
# (dev-bins workspace: cd dev-bins && cargo build -p agave-ledger-tool).
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
PROGRAM_DIR="$(cd "$HERE/../.." && pwd)"
REPO="$(cd "$PROGRAM_DIR/../.." && pwd)"
TV="$REPO/target/debug/solana-test-validator"
LT="$REPO/dev-bins/target/debug/agave-ledger-tool"
SO="$PROGRAM_DIR/target/deploy/cookie_feature_gate.so"
F=B1ockRevenueSharing111111111111111111111111
G=CustomCommissionCo11ector111111111111111111
H=bn1hKNURMGQaQoEVxahcEAcqiX3NwRs6hgKKNSLeKxH
RPC=http://127.0.0.1:8899
[ -x "$TV" ] && [ -x "$LT" ] && [ -f "$SO" ] || { echo "build the test validator, ledger-tool and program first"; exit 2; }

W="$(mktemp -d)"
trap 'kill $VPID 2>/dev/null; rm -rf "$W"' EXIT
UA="$W/ua.json"
solana-keygen new --no-bip39-passphrase --silent -o "$UA" >/dev/null
"$TV" --reset --quiet --ledger "$W/ledger" --slots-per-epoch 64 --deactivate-feature "$F" --deactivate-feature "$G" --deactivate-feature "$H" \
  --upgradeable-program CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4 "$SO" "$(solana-keygen pubkey "$UA")" > "$W/validator.out" 2>&1 &
VPID=$!
for _ in $(seq 1 120); do solana -u "$RPC" cluster-version >/dev/null 2>&1 && break; sleep 1; done
(cd "$HERE" && node --experimental-require-module --no-warnings e2e.js "$UA" "$F" "$G" "$H" cookie | tail -1)
for _ in $(seq 1 120); do [ "$(solana -u "$RPC" slot --commitment finalized 2>/dev/null || echo 0)" -gt 80 ] && break; sleep 1; done
kill $VPID; wait $VPID 2>/dev/null; sleep 2

"$LT" -l "$W/ledger" create-snapshot ROOT "$W/snapshot" --deactivate-feature-gate "$F" \
  --enable-capitalization-change > "$W/ledger-tool.out" 2>&1
RC=$?
if [ $RC -eq 0 ] && grep -q "Successfully created snapshot" "$W/ledger-tool.out" \
   && grep -q "Capitalization change: -953520 lamports" "$W/ledger-tool.out"; then
  echo "PASS  ledger-tool deactivated the alias-activated feature (alias account cleared)"
else
  echo "FAIL  ledger-tool could not deactivate the alias-activated feature"
  grep -E "Error|error" "$W/ledger-tool.out" | tail -3
  exit 1
fi
