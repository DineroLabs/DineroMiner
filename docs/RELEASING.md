# Releasing DineroMiner

## macOS (arm64)

1. Stage sidecars + lock: `./scripts/fetch-sidecars.sh`
   (source: `DINERO_BUILD_DIR`, default `~/src/dinero-v8/build`)
2. Build the signed bundle: `security unlock-keychain -p '' ~/Library/Keychains/login.keychain-db && cargo tauri build`
   — signs as `Developer ID Application: DineroLabs LLC (JXJS6ZA5FJ)` per tauri.conf.json.
3. Verify: `codesign -dvvv src-tauri/target/release/bundle/macos/DineroMiner.app | grep Authority`
   and launch it once (run the manual checklist from the v1 plan, Task 9).
4. Notarize + staple the DMG (mirrors DineroDPI's flow):
   ```
   xcrun notarytool submit src-tauri/target/release/bundle/dmg/DineroMiner_*.dmg \
     --keychain-profile dinero-notarytool --wait
   xcrun stapler staple src-tauri/target/release/bundle/dmg/DineroMiner_*.dmg
   spctl -a -vvv -t install src-tauri/target/release/bundle/dmg/DineroMiner_*.dmg  # accepted, Notarized Developer ID
   ```
5. GitHub release: tag `vX.Y.Z` on main, attach the DMG, put SHA-256 sums in the
   notes, mark Latest — same convention as DineroDPI releases.

## Known gaps (v1)

- **Windows/Linux sidecars:** `fetch-sidecars.sh` currently sources the local
  macOS dinero-v8 build only. Wire the dinero release tarballs
  (linux-x86_64, linux-aarch64 for Chromebook/Crostini, windows-x64) into the
  script before shipping those platforms; the SV2 pool miners additionally
  need dinero-sv2 cross-builds.
- **Pool key rotation:** the app pins the SV2 pool's static pubkey
  (`worksource::DEFAULT_POOL_PUBKEY`). If the pool key rotates, ship an
  updated default (users can also override `pool_pubkey` in settings.json).
- **Pool health:** if miners report instant session drops and stale job
  heights, the pool's template loop is wedged — `systemctl restart
  dinero-sv2-pool` on SJ fixes it (seen 2026-08-14).
