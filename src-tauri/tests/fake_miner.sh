#!/bin/sh
# Modes via $1: "steady" prints stats forever; "crash" exits immediately;
# "burst" prints 3 lines then exits 0.
case "$1" in
  steady) while true; do echo "hashrate: 123 kH/s"; echo "✅ Share accepted"; sleep 0.1; done ;;
  burst)  echo "hashrate: 5 H/s"; echo "unknown gibberish"; echo "✅ Share accepted"; exit 0 ;;
  crash)  echo "error: boom" >&2; exit 1 ;;
esac
