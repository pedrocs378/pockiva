#!/usr/bin/env bash
set -euo pipefail

acceptance=mooneye-mit-31510e1+dmg-acid2-mit-v1.0
if [[ "${GB_PPU_ROM_ASSET_ACCEPT:-}" != "$acceptance" ]]; then
  printf 'Fetch the pinned MIT Mooneye 31510e1 and dmg-acid2 v1.0 assets? Type yes: '
  read -r response
  [[ "$response" == yes ]] || { echo "Asset fetch declined."; exit 2; }
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
download_root="$script_dir/downloads"
mooneye_url=https://gekkio.fi/files/mooneye-test-suite/mts-20260714-0944-31510e1/mts-20260714-0944-31510e1.tar.xz
mooneye_hash=6d4fdda2f1d8d2f5f51b0ff3f6f3cc2fbae047aa395a39c82bda3a0e7cbd2641
acid2_url=https://github.com/mattcurrie/dmg-acid2/releases/download/v1.0/dmg-acid2.gb
acid2_hash=464e14b7d42e7feea0b7ede42be7071dc88913f75b9ffa444299424b63d1dff1

echo "Mooneye Test Suite 31510e12eea6286d36eea060a6adde755e1067aa (MIT)"
echo "License: https://github.com/Gekkio/mooneye-test-suite/blob/31510e12eea6286d36eea060a6adde755e1067aa/LICENSE"
echo "Source: $mooneye_url"
echo "Archive SHA-256: $mooneye_hash"
echo "dmg-acid2 v1.0 / dc22954 (MIT)"
echo "License: https://github.com/mattcurrie/dmg-acid2/blob/dc22954/LICENSE"
echo "Source: $acid2_url"
echo "ROM SHA-256: $acid2_hash"

temp_root=$(mktemp -d)
trap 'rm -rf "$temp_root"' EXIT
archive="$temp_root/mooneye.tar.xz"
acid2="$temp_root/dmg-acid2.gb"
extract_root="$temp_root/extract"
mkdir -p "$extract_root"

curl --fail --location --proto '=https' --tlsv1.2 "$mooneye_url" --output "$archive"
curl --fail --location --proto '=https' --tlsv1.2 "$acid2_url" --output "$acid2"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}';
  else shasum -a 256 "$1" | awk '{print $1}'; fi
}
[[ "$(hash_file "$archive")" == "$mooneye_hash" ]] || { echo "Mooneye archive checksum mismatch"; exit 1; }
[[ "$(hash_file "$acid2")" == "$acid2_hash" ]] || { echo "dmg-acid2 checksum mismatch"; exit 1; }

archive_entries=()
while IFS= read -r entry; do
  [[ "$entry" != /* && "$entry" != ../* && "$entry" != *'/../'* ]] || {
    echo "Unsafe archive path: $entry"; exit 1;
  }
  archive_entries+=("$entry")
done < <(tar -tJf "$archive")

relative_roms=(
  acceptance/ppu/hblank_ly_scx_timing-GS.gb
  acceptance/ppu/intr_1_2_timing-GS.gb
  acceptance/ppu/intr_2_0_timing.gb
  acceptance/ppu/intr_2_mode0_timing.gb
  acceptance/ppu/intr_2_mode0_timing_sprites.gb
  acceptance/ppu/intr_2_mode3_timing.gb
  acceptance/ppu/intr_2_oam_ok_timing.gb
  acceptance/ppu/lcdon_timing-GS.gb
  acceptance/ppu/lcdon_write_timing-GS.gb
  acceptance/ppu/stat_irq_blocking.gb
  acceptance/ppu/stat_lyc_onoff.gb
  acceptance/ppu/vblank_stat_intr-GS.gb
)
archive_members=()
for relative in "${relative_roms[@]}"; do
  matches=()
  for entry in "${archive_entries[@]}"; do
    [[ "$entry" == "$relative" || "$entry" == */"$relative" ]] && matches+=("$entry")
  done
  [[ "${#matches[@]}" == 1 ]] || { echo "Expected one archive member for $relative"; exit 1; }
  archive_members+=("${matches[0]}")
done
tar -xJf "$archive" -C "$extract_root" -- "${archive_members[@]}"

mkdir -p "$download_root/mooneye" "$download_root/dmg-acid2"
for index in "${!relative_roms[@]}"; do
  relative=${relative_roms[$index]}
  member=${archive_members[$index]}
  mkdir -p "$download_root/mooneye/$(dirname -- "$relative")"
  cp "$extract_root/$member" "$download_root/mooneye/$relative"
done
cp "$acid2" "$download_root/dmg-acid2/dmg-acid2.gb"
"$script_dir/verify-roms.sh" --all
