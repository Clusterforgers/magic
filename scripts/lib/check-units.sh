#!/usr/bin/env bash
# shellcheck disable=SC2034,SC2154 # the libraries share globals with check.sh
# Test binaries as units of scripts/check.sh: reads the artifacts of `cargo test --no-run --message-format=json`.
# Sourced by check.sh after check-plan.sh and check-run.sh.

# unit_crate <unit>: the crate a unit tests; crate_left <crate>: its units still running.
declare -A unit_crate=() crate_left=()
REPLY_WORDS=()

# slice_words <exe> <unit> <skip args...>: sets REPLY_WORDS to the exact names of the tests the unit runs in this seed
# slice (the slow list applied as well) and prints how many it runs; fails when it runs none.
slice_words() {
  local exe="$1" unit="$2" skips="" all kept
  shift 2
  while (($#)); do
    if [[ "$1" == --skip ]] && (($# > 1)); then
      skips+="$2"$'\n'
      shift
    fi
    shift
  done
  all=$("$exe" --list --format terse 2>/dev/null | sed -n 's/: test$//p')
  mapfile -t REPLY_WORDS < <(printf '%s\n' "$all" | sed '/^$/d' | slice_tests "$unit" "$seed_slice" "$skips")
  kept=${#REPLY_WORDS[@]}
  echo "$unit: seed slice $seed_slice runs $kept of $(printf '%s\n' "$all" | sed '/^$/d' | wc -l) tests"
  ((kept > 0))
}

# One line, because the words of a unit are kept one per line.
# shellcheck disable=SC2016 # the script runs in a child shell
web_script='pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format && if [ "$1" = full ]; then pnpm --silent test; else pnpm exec vitest run --changed "$2" --passWithNoTests; fi'

# add_web_unit <web dir> <full|changed> <base>: the web checks, with every test or only those a diff against base touches.
add_web_unit() {
  add_unit web web "$1" "$web_threads" "" "$(printf '%s\n' bash -c "$web_script" _ "$2" "$3")"
}

# unit_secs_file: the seconds of the last passing run of each unit, one "<unit> <seconds>" line per run.
unit_secs_file="${stamp_file%/*}/unit-secs"

# record_unit_secs: remember how long each test binary of the run took, once the scheduler is done (the newest 500
# runs are kept). Only units that passed are in unit_secs then: the first failure ends the check.
record_unit_secs() {
  local name
  mkdir -p "${unit_secs_file%/*}"
  for name in "${!unit_secs[@]}"; do
    [[ "$name" == web ]] || echo "$name ${unit_secs[$name]}"
  done >>"$unit_secs_file"
  if (($(wc -l <"$unit_secs_file") > 1000)); then
    tail -n 500 "$unit_secs_file" >"$unit_secs_file.tmp" && mv "$unit_secs_file.tmp" "$unit_secs_file"
  fi
}

# unit_name <crate> <target> <target kind>: the name of a test binary as a unit (sets REPLY).
unit_name() {
  case "$3" in
    lib) REPLY="$1::lib" ;;
    bin) REPLY="$1::bin:$2" ;;
    *) REPLY="$1::$2" ;;
  esac
}

# add_test_units <json log> <skip args...>: one unit per test binary of the log, the longest first by the seconds of
# its last run, then the biggest; the first takes the most threads. A binary runs from its crate folder with the
# environment cargo gives a test, so the feature set of the one build is the one tested.
add_test_units() {
  local json="$1" root kind dir target tkind exe id crate ver name env_lines row
  local -A crate_of_dir=() bin_env=() last_secs=()
  local -a rows=() sorted=() words
  shift
  root=$(pwd)
  for crate in "${all_crates[@]}"; do crate_of_dir["$root/${crate_dir[$crate]}"]="$crate"; done
  if [[ -s "$unit_secs_file" ]]; then
    while read -r name row; do last_secs[$name]="$row"; done <"$unit_secs_file"
  fi
  mapfile -t rows < <(jq -r 'select(.reason == "compiler-artifact" and .executable != null)
    | [(if .profile.test then "test" else "bin" end), (.manifest_path | rtrimstr("/Cargo.toml")), .target.name,
       .target.kind[0], .executable, .package_id] | @tsv' "$json")
  for row in "${rows[@]}"; do
    IFS=$'\t' read -r kind dir target tkind exe id <<<"$row"
    if [[ "$kind" == bin && "$tkind" == bin ]]; then
      bin_env[$dir]+="CARGO_BIN_EXE_$target=$exe"$'\n'
    elif [[ "$kind" == test ]]; then
      unit_name "${crate_of_dir[$dir]:-}" "$target" "$tkind"
      sorted+=("${last_secs[$REPLY]:-0}"$'\t'"$(stat -c %s "$exe")"$'\t'"$row")
    fi
  done
  ((${#sorted[@]} == 0)) || mapfile -t sorted < <(printf '%s\n' "${sorted[@]}" | sort -t$'\t' -k1,1rn -k2,2rn)
  for row in "${sorted[@]}"; do
    IFS=$'\t' read -r _ _ kind dir target tkind exe id <<<"$row"
    crate="${crate_of_dir[$dir]:-}"
    [[ -n "$crate" ]] || continue
    ver="${id##*#}"
    ver="${ver##*@}"
    unit_name "$crate" "$target" "$tkind"
    name="$REPLY"
    [[ -z "${last_secs[$name]:-}" ]] || u_expect[$name]="${last_secs[$name]}"
    env_lines="CARGO_MANIFEST_DIR=$dir"$'\n'"CARGO_PKG_NAME=$crate"$'\n'"CARGO_PKG_VERSION=$ver"$'\n'"RUSTC_BOOTSTRAP=1"
    env_lines+=$'\n'"${bin_env[$dir]:-}"
    words=("$exe" -Z unstable-options --report-time "$@")
    case "$tkind" in
      lib) unit_rerun[$name]="cargo test -p $crate --lib --" ;;
      bin) unit_rerun[$name]="cargo test -p $crate --bin $target --" ;;
      *) unit_rerun[$name]="cargo test -p $crate --test $target --" ;;
    esac
    if [[ "${seed_slice:-all}" != all ]] && sliced_unit "$name"; then
      slice_words "$exe" "$name" "$@" || continue
      words=("$exe" -Z unstable-options --report-time --exact "${REPLY_WORDS[@]}")
      unit_sliced[$name]=1
    fi
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
