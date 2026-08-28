#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" != "--mooneye" ]]; then
  echo "only --mooneye may be fetched; Blargg is local-only and is never downloaded" >&2
  exit 2
fi

revision="31510e12eea6286d36eea060a6adde755e1067aa"
license_url="https://github.com/Gekkio/mooneye-test-suite/blob/$revision/LICENSE"
archive_url="https://gekkio.fi/files/mooneye-test-suite/mts-20260714-0944-31510e1/mts-20260714-0944-31510e1.tar.xz"
archive_sha256="6d4fdda2f1d8d2f5f51b0ff3f6f3cc2fbae047aa395a39c82bda3a0e7cbd2641"
repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
destination="$repository_root/tests/roms/downloads/mooneye"
temporary_directory="$(mktemp -d)"
trap 'rm -rf "$temporary_directory"' EXIT

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "missing SHA-256 tool: install sha256sum or shasum" >&2
    exit 2
  fi
}

echo "Project: Mooneye Test Suite"
echo "Revision: $revision"
echo "License: MIT ($license_url)"
echo "Archive: $archive_url"
echo "Archive SHA-256: $archive_sha256"

if [[ "${GB_ROM_ASSET_ACCEPT:-}" != "mooneye-mit-31510e1" ]]; then
  if [[ ! -t 0 ]]; then
    echo "non-interactive use requires GB_ROM_ASSET_ACCEPT=mooneye-mit-31510e1" >&2
    exit 2
  fi
  read -r -p "Type yes to download these redistributable test assets: " answer
  [[ "$answer" == "yes" ]] || { echo "download cancelled" >&2; exit 2; }
fi

archive="$temporary_directory/mooneye.tar.xz"
curl --fail --location --proto '=https' --tlsv1.2 "$archive_url" --output "$archive"
actual_archive_sha256="$(sha256_file "$archive")"
[[ "$actual_archive_sha256" == "$archive_sha256" ]] || {
  echo "archive checksum mismatch" >&2
  echo "expected $archive_sha256" >&2
  echo "actual   $actual_archive_sha256" >&2
  exit 1
}

extract_root="$temporary_directory/extracted"
mkdir -p "$extract_root"
tar -xJf "$archive" -C "$extract_root"
suite_root="$(find "$extract_root" -type d -name acceptance -print -quit | xargs dirname)"
[[ -n "$suite_root" ]] || { echo "archive does not contain acceptance tests" >&2; exit 1; }

paths=(
  acceptance/bits/reg_f.gb
  acceptance/instr/daa.gb
  acceptance/ei_sequence.gb
  acceptance/if_ie_registers.gb
  acceptance/timer/div_write.gb
  acceptance/timer/tima_reload.gb
  acceptance/oam_dma/basic.gb
)
for relative_path in "${paths[@]}"; do
  source_path="$suite_root/$relative_path"
  [[ -f "$source_path" ]] || { echo "archive missing $relative_path" >&2; exit 1; }
  mkdir -p "$destination/$(dirname "$relative_path")"
  cp "$source_path" "$destination/$relative_path"
done

"$repository_root/scripts/verify-core-test-roms.sh" --mooneye
