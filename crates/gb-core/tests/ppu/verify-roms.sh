#!/usr/bin/env bash
set -u

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
download_root="$script_dir/downloads"

hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

verify() {
  local path=$1 expected=$2 actual
  if [[ ! -f "$path" ]]; then
    echo "MISSING ${path#"$download_root/"}"
    return 1
  fi
  actual=$(hash_file "$path")
  if [[ "$actual" != "$expected" ]]; then
    echo "MISMATCH ${path#"$download_root/"}: expected $expected, received $actual"
    return 1
  fi
  echo "VERIFIED ${path#"$download_root/"}"
}

verify_mooneye() {
  local failed=0
  while read -r relative expected; do
    verify "$download_root/mooneye/$relative" "$expected" || failed=1
  done <<'ROMS'
acceptance/ppu/hblank_ly_scx_timing-GS.gb 3adec9174d16b7a4cece42e5525e4363ff956c19070600aa9344de68b0885449
acceptance/ppu/intr_1_2_timing-GS.gb 3bac47fc79ce514fd7f6bbe0d87f1160b91a5292be27fee7bc3bcea6bc171ee9
acceptance/ppu/intr_2_0_timing.gb 6ea58d6940ad2dde6d20ef1fc63f1da83bdff842672d757a7a2377a3d0cfb7ff
acceptance/ppu/intr_2_mode0_timing.gb be1555d577506073ba1ec4717060aa24075c02b9c787b874623a98bf2ac2da6e
acceptance/ppu/intr_2_mode0_timing_sprites.gb 52b10bb0d3073ec35d6bc4f0129fcabb788e4d11ea765163a49d519121d5169e
acceptance/ppu/intr_2_mode3_timing.gb b5cb7d22162e3ed6fa2dafeaa487cf1d1c042b5e8a3a9877823c33b578b9c31e
acceptance/ppu/intr_2_oam_ok_timing.gb 38d7acfddce357c8b084f9bb647d6ffc99d1fb85d7a312c2db2c348ba888f7ff
acceptance/ppu/lcdon_timing-GS.gb 2a9d46b61935ae1a2332abd419bd6d63c2c48697b96ad547c859c207cf531e2f
acceptance/ppu/lcdon_write_timing-GS.gb e28b34cef8b5d58bf19e058be2206309129a5896568e918b6b11b6c61dce2a51
acceptance/ppu/stat_irq_blocking.gb 604436aeb6a37badd71be0fafa526307345f1de6af757193f11fc77e09a01fc7
acceptance/ppu/stat_lyc_onoff.gb 29f04aaf6b26085bca1dccfab648fb44fbf57d4aa923bca75a30167e45d8670e
acceptance/ppu/vblank_stat_intr-GS.gb f7de9a3ef1399f73ad16ef23dccf05d38cbd62373215608ee5da53a35850436e
ROMS
  return "$failed"
}

verify_acid2() {
  verify "$download_root/dmg-acid2/dmg-acid2.gb" 464e14b7d42e7feea0b7ede42be7071dc88913f75b9ffa444299424b63d1dff1
}

case "${1:-}" in
  --mooneye) verify_mooneye ;;
  --dmg-acid2) verify_acid2 ;;
  --all)
    failed=0
    verify_mooneye || failed=1
    verify_acid2 || failed=1
    exit "$failed"
    ;;
  *) echo "usage: $0 --mooneye|--dmg-acid2|--all" >&2; exit 2 ;;
esac
