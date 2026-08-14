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

HASH=$("$B/dinero-cli" -regtest -datadir="$D" getbestblockhash)
BLOCK=$("$B/dinero-cli" -regtest -datadir="$D" getblock "$HASH" 2)
echo "$BLOCK" | grep -q "$ADDR" || {
  echo "FAIL: coinbase does not pay $ADDR"
  echo "$BLOCK" | head -40
  exit 1
}
echo "PASS: block $H coinbase pays $ADDR"
