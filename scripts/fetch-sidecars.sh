#!/usr/bin/env bash
# Populates src-tauri/binaries/ for the current platform and (re)writes the
# entries in scripts/sidecars.lock. v1 source: the local dinero-v8 build for
# macOS; Linux/Windows artifacts come from the dinero release tarballs and are
# added to this script when those bundles are wired (documented gap until then).
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p src-tauri/binaries
SRC="${DINERO_BUILD_DIR:-$HOME/src/dinero-v8/build}"
SV2="${SV2_BUILD_DIR:-$HOME/src/dinero-sv2/target/release}"
: > scripts/sidecars.lock.new
rm -f src-tauri/binaries/dinero-stratum-worker  # legacy stratum: nothing serves it
# Solo miners (node-RPC) from dinero-v8; pool miners (SV2/Noise) from dinero-sv2.
for b in dinero-miner dinero-gpu-miner; do
  cp "$SRC/$b" src-tauri/binaries/
  shasum -a 256 "src-tauri/binaries/$b" | sed 's|src-tauri/binaries/||' >> scripts/sidecars.lock.new
done
for b in dinero-sv2-miner dinero-sv2-gpu-miner; do
  cp "$SV2/$b" src-tauri/binaries/
  shasum -a 256 "src-tauri/binaries/$b" | sed 's|src-tauri/binaries/||' >> scripts/sidecars.lock.new
done
sort -u scripts/sidecars.lock.new > scripts/sidecars.lock && rm scripts/sidecars.lock.new
cp scripts/sidecars.lock src-tauri/binaries/sidecars.lock   # bundled as a resource
echo "sidecars staged; lock updated"
