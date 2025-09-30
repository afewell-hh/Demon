#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

bundle_output="/tmp/contracts-bundle.json"
echo "[contracts-validate] Generating contracts bundle schema to ${bundle_output}"
cargo run -p demonctl -- contracts bundle --format json --include-wit >"${bundle_output}"

shopt -s nullglob
declare -a spec_files=(engine/tests/*_contracts_spec.rs)
if [[ ${#spec_files[@]} -eq 0 ]]; then
  echo "[contracts-validate] No contract spec tests found."
  exit 0
fi

for spec in "${spec_files[@]}"; do
  test_name="$(basename "${spec}")"
  test_name="${test_name%.rs}"
  echo "[contracts-validate] Running cargo test -p engine --test ${test_name}"
  cargo test -p engine --test "${test_name}"
done
