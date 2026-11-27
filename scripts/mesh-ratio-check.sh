#!/usr/bin/env bash
# usage: mesh-ratio-check.sh <num_gen> <den_gen> <max_ratio>
# Reads criterion medians of the mesh_build group; run `rm -rf target/criterion/mesh_build`
# and `cargo bench -p planetary-core --bench mesh` first so stale estimates cannot pass.
set -u
num=$1
den=$2
max=$3
root="${CARGO_TARGET_DIR:-target}/criterion"
rc=0

median() {
  local id=$1 gen=$2 f
  f=$(find "$root" -path "*${id}?gen${gen}/new/estimates.json" | head -n 1)
  [ -n "$f" ] || return 0
  tr -d '[:space:]' < "$f" | sed -n 's/.*"median":{"confidence_interval":{[^}]*},"point_estimate":\([0-9.eE+-]*\).*/\1/p'
}

for id in "hex_cell?hexagon_l8_n16" "hex_cell?pentagon_l8_n16" "cube_patch?level4_n32" "cube_patch?level10_n32"; do
  n=$(median "$id" "$num")
  d=$(median "$id" "$den")
  if [ -z "$n" ] || [ -z "$d" ]; then
    echo "MISSING estimate for $id"
    rc=1
    continue
  fi
  r=$(awk -v n="$n" -v d="$d" 'BEGIN{printf "%.3f", n/d}')
  echo "ratio gen$num/gen$den $id = $r (limit $max)"
  if awk -v r="$r" -v m="$max" 'BEGIN{exit !(r>m)}'; then rc=1; fi
done

if [ "$rc" -eq 0 ]; then echo "MESH RATIO PASS"; else echo "MESH RATIO FAIL"; fi
exit $rc
