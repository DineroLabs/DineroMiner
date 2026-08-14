#!/usr/bin/env bash
# E2E: solo-mine one regtest block via the same sidecar+args the app resolves,
# then assert the coinbase pays the target address.
set -euo pipefail
B="${DINERO_BUILD_DIR:-$HOME/src/dinero-v8/build}"
D=$(mktemp -d /tmp/dm-e2e.XXXXXX)
cleanup() {
  "$B/dinero-cli" -regtest -datadir="$D" stop >/dev/null 2>&1 || true
  pkill -f "dinerod -regtest -datadir=$D" 2>/dev/null || true
  sleep 1
  rm -rf "$D"
}
trap cleanup EXIT

# dinerod serves RPC only with the explicit --rpc flag.
"$B/dinerod" -regtest -datadir="$D" --rpc --rpcport 20996 >/dev/null 2>&1 &
for i in $(seq 1 30); do
  "$B/dinero-cli" -regtest -datadir="$D" getblockcount >/dev/null 2>&1 && break
  sleep 2
done
ADDR=$("$B/dinero-cli" -regtest -datadir="$D" getnewaddress \
  | python3 -c "import json,sys; print(json.load(sys.stdin)['address'])")
H0=$("$B/dinero-cli" -regtest -datadir="$D" getblockcount)

timeout 120 "$B/dinero-miner" --rpcport 20996 --datadir "$D" --address "$ADDR" --threads 2 --force >/dev/null 2>&1 &
MINER=$!
H=$H0
for i in $(seq 1 60); do
  H=$("$B/dinero-cli" -regtest -datadir="$D" getblockcount)
  [ "$H" -gt "$H0" ] && break
  sleep 2
done
kill $MINER 2>/dev/null || true
[ "$H" -gt "$H0" ] || { echo "FAIL: no block mined"; exit 1; }

# getbestblockhash returns a JSON-quoted string; getblock verbosity 2 lists
# txids only (no decoded outputs), so the coinbase-pays-address assertion goes
# through the wallet ledger: the fresh datadir's wallet owns exactly one
# address ($ADDR), so any immature coinbase balance proves the coinbase paid it.
HASH=$("$B/dinero-cli" -regtest -datadir="$D" getbestblockhash | tr -d '"')
"$B/dinero-cli" -regtest -datadir="$D" getblock "$HASH" 2 | grep -q '"height"' || {
  echo "FAIL: best block $HASH not retrievable"
  exit 1
}
IMMATURE=$("$B/dinero-cli" -regtest -datadir="$D" getbalance \
  | python3 -c "import json,sys; print(json.load(sys.stdin).get('immature', 0))")
python3 -c "import sys; sys.exit(0 if float('$IMMATURE') > 0 else 1)" || {
  echo "FAIL: wallet holding $ADDR has no immature coinbase balance"
  exit 1
}
echo "PASS: block $H mined; coinbase credited $IMMATURE DIN (immature) to the wallet owning $ADDR"
