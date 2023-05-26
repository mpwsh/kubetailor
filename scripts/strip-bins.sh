#!/bin/bash
# From: https://github.com/holaplex/indexer/blob/dev/scripts/strip-bins.sh
# Author: ryans <https://github.com/ray-kast>
##

set -e

function is-elf() {
  readelf -h "$1" >/dev/null 2>/dev/null
}

mkdir -p "$2"

for f in "$1"/*; do
  is-elf "$f" || continue

  strip "$f"
  cp "$f" "$2/$(basename "$f")"
done
