#!/usr/bin/env bash
# Usage: scripts/check.sh [fast|rust|web|all|affected [BASE]|gate [BASE]] [--force] [--list]   (default: all)
#   fast: fmt + clippy + script tests   rust: fast + cargo tests   web: web checks only   all: everything
#   affected: fast, then only the tests a diff against BASE (default: the merge-base with origin/main) can break
#     (the orchestrator, once per batch; slow tests included).
#   gate: for agents, every commit: fmt, clippy and tests of the crates the diff changes, `cargo check` of the crates
#     that depend on them, the web checks when web/ changed; the tests of scripts/slow-tests.txt are skipped.
#   `--list` prints the chosen phases and exits (`affected --list BASE HEAD` lists the diff of two commits).
# A phase that passed on the same content (sources of a crate and of its workspace dependencies, Cargo.lock, toolchain)
# leaves a stamp in ~/.cache/mtg-check; every worktree with that content skips it unless --force.
# Test binaries run side by side within a thread budget: the cores (MTG_TEST_THREADS) split among the checks running,
# a quarter of the share per binary (RUST_TEST_THREADS overrides); a binary is killed after MTG_TEST_TIMEOUT minutes
# (default 40; a float is fine). CARGO_BUILD_JOBS (default 4) bounds the compiles.
set -euo pipefail
cd "$(dirname "$0")/.."

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
# shellcheck source=lib/check-plan.sh
source scripts/lib/check-plan.sh
# shellcheck source=lib/check-run.sh
source scripts/lib/check-run.sh
# shellcheck source=lib/check-units.sh
source scripts/lib/check-units.sh

mode=all force=0 list=0 revs=()
for arg in "$@"; do
  case "$arg" in
    fast | rust | web | all | affected | gate) mode="$arg" ;;
    --force) force=1 ;;
    --list) list=1 ;;
    -*) echo "unknown option: $arg" >&2 && exit 2 ;;
    *) revs+=("$arg") ;;
  esac
done
if [[ "$mode" != affected && "$mode" != gate ]] && ((${#revs[@]} || list)); then
  echo "BASE and --list belong to the affected and gate modes" >&2
  exit 2
fi
[[ "$mode" != gate ]] || gate=1

phases=("fmt" "clippy" "check" "script tests" "test build" "tests" "lock wait" "web")
declare -A secs
trap 'kill_units; [[ -z "$run_dir" ]] || rm -rf "$run_dir"' EXIT
trap 'exit 130' INT TERM

# --- Diff --------------------------------------------------------------------------------------------------------

base="" head_rev="" nfiles=0
changed_files() {
  if [[ -n "$head_rev" ]]; then
    git diff --name-only "$base" "$head_rev"
  else
    git diff --name-only "$base"
    git ls-files --others --exclude-standard
  fi
}

# --- Stamps ------------------------------------------------------------------------------------------------------

snap="" test_kind=test web_key="" web_name="-"

# passed <kind> <name>: the phase passed before on this content.
passed() {
  [[ -n "$snap" ]] && ((!force)) && [[ -s "$stamp_file" ]] || return 1
  stamp_line "$snap" "$1" "$2"
  grep -qxF "$REPLY" "$stamp_file"
}

# stamp <kind> <name>: record that the phase passed on this content.
stamp() {
  [[ -n "$snap" ]] || return 0
  stamp_line "$snap" "$1" "$2"
  mkdir -p "$(dirname "$stamp_file")"
  echo "$REPLY" >>"$stamp_file"
  if (($(wc -l <"$stamp_file") > 5000)); then
    tail -n 2000 "$stamp_file" >"$stamp_file.tmp" && mv "$stamp_file.tmp" "$stamp_file"
  fi
}

# --- Plan --------------------------------------------------------------------------------------------------------

other_sel=() pg_sel=() clippy_sel=() check_sel=()
declare -A tested

[[ "$mode" == web ]] || load_graph
case "$mode" in
  rust | all) for name in "${all_crates[@]}"; do hit[$name]=1; done ;;
  affected | gate)
    base="${revs[0]:-$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main)}"
    base=$(git rev-parse --verify "$base^{commit}")
    head_rev="${revs[1]:-}"
    while IFS= read -r f; do
      [[ -n "$f" ]] || continue
      classify "$f"
      nfiles=$((nfiles + 1))
    done < <(changed_files)
    if ((select_all)); then
      for name in "${all_crates[@]}"; do hit[$name]=1; done
    fi
    # The web sees only the bindings: a change elsewhere that alters them also changes the generated files.
    [[ -v hit[mtg-protocol] ]] && web_sel=full
    if ((gate)); then
      for name in "${!hit[@]}"; do tested[$name]=1; done
      close_over_dependents
      for name in "${all_crates[@]}"; do
        [[ -v hit[$name] && ! -v tested[$name] ]] && check_sel+=("$name")
        [[ -v tested[$name] ]] || unset "hit[$name]"
      done
    else
      close_over_dependents
    fi
    ;;
esac
[[ "$mode" == all || "$mode" == web ]] && web_sel=full
[[ -d web ]] || web_sel=none
((gate)) && test_kind=gate-test

# The content of the working tree names what was checked; a diff of two commits checks nothing.
if [[ -z "$head_rev" ]] && { ((!list)) || [[ -s "$stamp_file" ]]; }; then
  snap=$(snapshot_tree) || snap=""
fi

if [[ "$mode" != web ]]; then
  for name in "${all_crates[@]}"; do
    if ((gate)); then [[ -v hit[$name] ]] || continue; fi
    passed clippy "$name" || clippy_sel+=("$name")
  done
  checked=()
  for name in "${check_sel[@]}"; do
    passed check "$name" || checked+=("$name")
  done
  check_sel=("${checked[@]}")
  for name in "${all_crates[@]}"; do
    [[ -v hit[$name] ]] || continue
    if passed "$test_kind" "$name" || { ((gate)) && passed test "$name"; }; then
      echo "tests of $name: skipped: content already passed"
      continue
    fi
    if [[ "$pg_crates" == *" $name "* ]]; then pg_sel+=("$name"); else other_sel+=("$name"); fi
  done
fi
if [[ "$web_sel" != none ]]; then
  if [[ "$web_sel" == full ]]; then
    web_key=web web_name=-
  else
    tree_at "$base" web
    web_key=web-changed web_name="$REPLY"
  fi
  if passed "$web_key" "$web_name" || { [[ "$web_key" == web-changed ]] && passed web -; }; then
    echo "web: skipped: content already passed"
    web_sel=none
  fi
fi

print_plan() {
  [[ -z "$base" ]] || echo "base: $base"
  [[ -z "$base" ]] || echo "changed files: $nfiles"
  echo "fmt"
  echo "clippy: ${clippy_sel[*]:--}"
  ((!gate)) || echo "check: ${check_sel[*]:--}"
  echo "script tests"
  echo "tests without Postgres: ${other_sel[*]:--}"
  echo "tests with Postgres: ${pg_sel[*]:--}"
  echo "web: $web_sel"
  ((!gate)) || echo "slow tests skipped: $(slow_entries | wc -l) entries of $slow_file"
}

if ((list)); then
  print_plan
  exit 0
fi
[[ "$mode" != gate && "$mode" != affected ]] || print_plan

# --- Run ---------------------------------------------------------------------------------------------------------

# phase <name> <command...>: run the command and record its seconds.
phase() {
  local name="$1" start=$SECONDS
  shift
  "$@"
  secs[$name]=$((SECONDS - start))
}

# pkg_args <crate...>: `-p` arguments for cargo.
pkg_args() {
  local name
  for name in "$@"; do printf -- '-p\n%s\n' "$name"; done
}

# scope_args <crate...>: `--workspace` when every crate is named, else one `-p` per crate (sets scope).
scope_args() {
  if (($# == ${#all_crates[@]})); then scope=(--workspace); else mapfile -t scope < <(pkg_args "$@"); fi
}

# lint_crates <cargo command> <crates...>: clippy (with `-D warnings`) or check over the crates, then stamp them.
lint_crates() {
  local cmd="$1" crate scope=() tail=() start=$SECONDS
  shift
  if (($# == 0)); then
    echo "$cmd: nothing to do"
    return
  fi
  [[ "$cmd" != clippy ]] || tail=(-- -D warnings)
  scope_args "$@"
  cargo "$cmd" "${scope[@]}" --all-targets --quiet "${tail[@]}"
  for crate in "$@"; do stamp "$cmd" "$crate"; done
  secs[$cmd]=$((SECONDS - start))
}

script_tests() {
  local t
  for t in scripts/tests/*.sh; do "$t"; done
}

# on_unit_ok <unit>: a crate whose binaries all passed leaves its stamp; so does the web.
on_unit_ok() {
  local name="$1" crate="${unit_crate[$1]:-}"
  if [[ "$name" == web ]]; then
    stamp "$web_key" "$web_name"
  elif [[ -n "$crate" ]]; then
    crate_left[$crate]=$((crate_left[$crate] - 1))
    if ((crate_left[$crate] == 0)); then stamp "$test_kind" "$crate"; fi
  fi
}

web_script='pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format &&
  if [ "$1" = full ]; then pnpm --silent test; else pnpm exec vitest run --changed "$2" --passWithNoTests; fi'

root=$(pwd)
take_lease
init_units
# The web checks do not touch cargo: start them first, they run while the Rust phases compile.
if [[ "$web_sel" != none ]]; then
  add_unit web web "$root/web" "$web_threads" "" "$(printf '%s\n' bash -c "$web_script" _ "$web_sel" "$base")"
  start_ready
fi

if [[ "$mode" != web ]]; then
  phase "fmt" cargo fmt --all --check
  reap_done
  lint_crates clippy "${clippy_sel[@]}"
  reap_done
  lint_crates check "${check_sel[@]}"
  reap_done
  phase "script tests" script_tests
fi

skip_args=()
((!gate)) || mapfile -t skip_args < <(slow_skip_args)
if ((${#other_sel[@]} + ${#pg_sel[@]})); then
  json="$run_dir/build.json"
  mapfile -t pkgs < <(pkg_args "${other_sel[@]}" "${pg_sel[@]}")
  phase "test build" cargo test "${pkgs[@]}" --quiet --no-run --message-format=json-render-diagnostics >"$json"
  reap_done
  if ((${#pg_sel[@]})); then
    # Reuse a Postgres that is already listening (e.g. another worktree started one).
    if command -v docker >/dev/null && [[ -f docker-compose.yml ]] && ! (exec 3<>/dev/tcp/127.0.0.1/5432) 2>/dev/null; then
      docker compose up -d --wait db >/dev/null
    fi
    export DATABASE_URL="${DATABASE_URL:-postgres://magic:magic@localhost:5432/magic}"
  fi
  add_test_units "$json" "${skip_args[@]}"
fi
tests_start=$SECONDS
run_units
secs["tests"]=$((SECONDS - tests_start))
report_units

for name in "${phases[@]}"; do
  [[ -v secs[$name] ]] && echo "$name: ${secs[$name]}s"
done
echo "check ($mode): ok"
