#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
download_root="$repository_root/tests/roms/downloads"

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

verify_file() {
  local suite="$1"
  local relative_path="$2"
  local expected="$3"
  local asset="$download_root/$suite/$relative_path"
  if [[ ! -f "$asset" ]]; then
    if [[ "$suite" == "blargg" ]]; then
      echo "missing local-only Blargg asset: $asset" >&2
    else
      echo "missing explicitly provisioned Mooneye asset: $asset" >&2
    fi
    return 1
  fi
  local actual
  actual="$(sha256_file "$asset")"
  if [[ "$actual" != "$expected" ]]; then
    echo "checksum mismatch: $asset" >&2
    echo "expected $expected" >&2
    echo "actual   $actual" >&2
    return 1
  fi
  echo "verified $suite/$relative_path"
}

verify_mooneye() {
  verify_file mooneye acceptance/bits/reg_f.gb 4b193e887ee3ac82b38b796729e1503e9a78da3e1140f8bd5600d0884f2e2627
  verify_file mooneye acceptance/instr/daa.gb 1498d92d70592a07a2493ef764609916616f0b023b21408189e277201e6c14c1
  verify_file mooneye acceptance/ei_sequence.gb dcd7f37e8fe7d8eb38cab6732a5826e0bb0278fd1e1d9e297c28d205da1b69e1
  verify_file mooneye acceptance/if_ie_registers.gb d055b2b4c44902cf827296a06b17cea4f2c84f6b7d540c777cb1d1049ef35e61
  verify_file mooneye acceptance/timer/div_write.gb 2be1e4da6fa24b9123d2a8bae47dd0d6f5e97e1855186c0c0f49e6d213eebfff
  verify_file mooneye acceptance/timer/tima_reload.gb 1ca70c725bd1e027b07d3058839bd140eccddd9f4ca41305c4f8ab3acaff8a98
  verify_file mooneye acceptance/oam_dma/basic.gb 326b747cac8cc96b62d6ee508e73b87eda24bfe29553d3d32e719f3b6d76c97c
}

verify_blargg() {
  verify_file blargg cpu_instrs/cpu_instrs.gb 8c5e12f41e0ba5bbca796944f92ffe6de28809198682c4332e38d1b3cf56fcf2
  verify_file blargg instr_timing/instr_timing.gb 646067b3d6c79fda810e9c3f1cb7c0efd5abb0a7ac06437c54e65720c15d9925
}

case "${1:-}" in
  --mooneye) verify_mooneye ;;
  --blargg) verify_blargg ;;
  --all) verify_mooneye; verify_blargg ;;
  *) echo "usage: $0 --mooneye|--blargg|--all" >&2; exit 2 ;;
esac
