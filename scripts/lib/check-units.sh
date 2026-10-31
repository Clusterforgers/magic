#!/usr/bin/env bash
# shellcheck disable=SC2034,SC2154 # the libraries share globals with check.sh
# Test binaries as units of scripts/check.sh: reads the artifacts of `cargo test --no-run --message-format=json`.
# Sourced by check.sh after check-plan.sh and check-run.sh.

# unit_crate <unit>: the crate a unit tests; crate_left <crate>: its units still running.
declare -A unit_crate=() crate_left=()

# One line, because the words of a unit are kept one per line.
# shellcheck disable=SC2016 # the script runs in a child shell
web_script='pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format && if [ "$1" = full ]; then pnpm --silent test; else pnpm exec vitest run --changed "$2" --passWithNoTests; fi'

# add_web_unit <web dir> <full|changed> <base>: the web checks, with every test or only those a diff against base touches.
add_web_unit() {
  add_unit web web "$1" "$web_threads" "" "$(printf '%s\n' bash -c "$web_script" _ "$2" "$3")"
}

# add_test_units <json log> <skip args...>: one unit per test binary of the log, the biggest binary first. A binary runs
# from its crate folder with the environment cargo gives a test, so the feature set of the one build is the one tested.
add_test_units() {
  local json="$1" root kind dir target tkind exe id crate ver name env_lines row
  local -A crate_of_dir=() bin_env=()
  local -a rows=() sorted=() words
  shift
  root=$(pwd)
  for crate in "${all_crates[@]}"; do crate_of_dir["$root/${crate_dir[$crate]}"]="$crate"; done
  mapfile -t rows < <(jq -r 'select(.reason == "compiler-artifact" and .executable != null)
    | [(if .profile.test then "test" else "bin" end), (.manifest_path | rtrimstr("/Cargo.toml")), .target.name,
       .target.kind[0], .executable, .package_id] | @tsv' "$json")
  for row in "${rows[@]}"; do
    IFS=$'\t' read -r kind dir target tkind exe id <<<"$row"
    if [[ "$kind" == bin && "$tkind" == bin ]]; then
      bin_env[$dir]+="CARGO_BIN_EXE_$target=$exe"$'\n'
    elif [[ "$kind" == test ]]; then
      sorted+=("$(stat -c %s "$exe")"$'\t'"$row")
    fi
  done
  ((${#sorted[@]} == 0)) || mapfile -t sorted < <(printf '%s\n' "${sorted[@]}" | sort -rn)
  for row in "${sorted[@]}"; do
    IFS=$'\t' read -r _ kind dir target tkind exe id <<<"$row"
    crate="${crate_of_dir[$dir]:-}"
    [[ -n "$crate" ]] || continue
    ver="${id##*#}"
    ver="${ver##*@}"
    case "$tkind" in
      lib) name="$crate::lib" ;;
      bin) name="$crate::bin:$target" ;;
      *) name="$crate::$target" ;;
    esac
    env_lines="CARGO_MANIFEST_DIR=$dir"$'\n'"CARGO_PKG_NAME=$crate"$'\n'"CARGO_PKG_VERSION=$ver"$'\n'"RUSTC_BOOTSTRAP=1"
    env_lines+=$'\n'"${bin_env[$dir]:-}"
    words=("$exe" -Z unstable-options --report-time "$@")
    kind=tests
    [[ "$pg_crates" != *" $crate "* ]] || kind=pg
    add_unit "$name" "$kind" "$dir" auto "$env_lines" "$(printf '%s\n' "${words[@]}")"
    unit_crate[$name]="$crate"
    crate_left[$crate]=$((${crate_left[$crate]:-0} + 1))
  done
  # Doc tests need cargo itself; no crate has any today.
  for crate in "${!crate_left[@]}"; do
    grep -rqE '^\s*//[/!]\s*```' "${crate_dir[$crate]}/src" 2>/dev/null || continue
    kind=tests
    [[ "$pg_crates" != *" $crate "* ]] || kind=pg
    add_unit "$crate::doc" "$kind" "$root" auto "" "$(printf '%s\n' cargo test --doc -p "$crate" --quiet -- "$@")"
    unit_crate["$crate::doc"]="$crate"
    crate_left[$crate]=$((crate_left[$crate] + 1))
  done
}
