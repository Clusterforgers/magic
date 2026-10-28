#!/usr/bin/env bash
# Usage: scripts/check.sh [fast|rust|web|all|affected [BASE]] [--force] [--list]   (default: all)
#   fast: fmt + clippy + script tests   rust: fast + cargo tests   web: web checks only   all: everything
#   affected: fast, then only the tests a diff against BASE (default: the merge-base with origin/main) can break;
#     `--list` prints the chosen phases and exits (`affected --list BASE HEAD` lists the diff of two commits).
# A phase that passed on a clean tree leaves a stamp in the git common dir; the same tree skips it unless --force.
# The last lines give the seconds each phase took and the waits for the Postgres lock and for a test slot.
#
# Shared machine: compiles use CARGO_BUILD_JOBS (default 4); test runs take one of MTG_TEST_SLOTS slots (default 4,
# `/tmp/mtg-test-slot.N`, held machine-wide) with RUST_TEST_THREADS test threads each (default 6, so 4 x 6 = 24 cores)
# and are killed after MTG_TEST_TIMEOUT minutes (default 40; a float is fine). On an idle machine run e.g.
#   CARGO_BUILD_JOBS=24 MTG_TEST_SLOTS=1 RUST_TEST_THREADS=24 scripts/check.sh
set -euo pipefail
cd "$(dirname "$0")/.."

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-6}"
slots="${MTG_TEST_SLOTS:-4}"
test_timeout="${MTG_TEST_TIMEOUT:-40}"

mode=all force=0 list=0 revs=()
for arg in "$@"; do
  case "$arg" in
    fast | rust | web | all | affected) mode="$arg" ;;
    --force) force=1 ;;
    --list) list=1 ;;
    -*) echo "unknown option: $arg" >&2 && exit 2 ;;
    *) revs+=("$arg") ;;
  esac
done
if [[ "$mode" != affected ]] && ((${#revs[@]} || list)); then
  echo "BASE and --list belong to the affected mode" >&2
  exit 2
fi

phases=("fmt" "clippy" "script tests" "test build" "tests without Postgres" "tests with Postgres" "lock wait" "slot wait" "web")
declare -A secs
run_log=""
trap '[[ -z "$run_log" ]] || rm -f "$run_log"' EXIT

# Only these crates connect to Postgres (`sqlx::test`); the others run without the database or the lock.
pg_crates=" mtg-db mtg-server mtg-cli "

# --- Crate graph -------------------------------------------------------------------------------------------------

# Lines "<crate> <dir relative to the root> <workspace deps, comma-separated>" from `cargo metadata`.
crate_graph() {
  cargo metadata --no-deps --format-version 1 --offline |
    jq -r '.workspace_root as $root | [.packages[].name] as $members | .packages[]
      | [.name, (.manifest_path | ltrimstr($root + "/") | rtrimstr("/Cargo.toml")),
         ([.dependencies[].name | select(IN($members[]))] | unique | join(","))] | join(" ")'
}

declare -A crate_dir crate_deps
all_crates=()
load_graph() {
  local name dir deps
  while read -r name dir deps; do
    crate_dir[$name]="$dir"
    crate_deps[$name]="$deps"
    all_crates+=("$name")
  done < <(crate_graph)
}

# --- Selection (affected) ----------------------------------------------------------------------------------------

declare -A hit
select_all=0 web_sel=none

# classify <path>: mark what a changed file can break.
classify() {
  local f="$1" name
  case "$f" in
    Cargo.toml | Cargo.lock | rust-toolchain | rust-toolchain.toml | scripts/check.sh | .cargo/*) select_all=1 ;;
    .sqlx/* | docker-compose.yml) hit[mtg-db]=1 ;;
    package.json | pnpm-lock.yaml | pnpm-workspace.yaml | web/src/protocol/generated/* | web/package.json | \
      web/pnpm-lock.yaml | web/vite.config.ts | web/tsconfig*.json | web/src/test-setup.ts) web_sel=full ;;
    web/*) [[ "$web_sel" == full ]] || web_sel=changed ;;
    crates/*)
      for name in "${all_crates[@]}"; do
        [[ "$f" == "${crate_dir[$name]}"/* ]] && hit[$name]=1 && return
      done
      select_all=1
      ;;
  esac
}

# close_over_dependents: add every crate that depends on a selected crate, until nothing changes.
close_over_dependents() {
  local grew=1 name dep
  while ((grew)); do
    grew=0
    for name in "${all_crates[@]}"; do
      [[ -v hit[$name] ]] && continue
      for dep in ${crate_deps[$name]//,/ }; do
        if [[ -v hit[$dep] ]]; then
          hit[$name]=1 grew=1
          break
        fi
      done
    done
  done
}

base="" head_rev=""
changed_files() {
  if [[ -n "$head_rev" ]]; then
    git diff --name-only "$base" "$head_rev"
  else
    git diff --name-only "$base"
    git ls-files --others --exclude-standard
  fi
}

# --- Stamps ------------------------------------------------------------------------------------------------------

stamp_file="${MTG_CHECK_STAMPS:-$(git rev-parse --path-format=absolute --git-common-dir)/mtg-check-stamps}"
tree="" rust_tc="" node_v=""
# Stamps only for a clean tree: the tree hash of HEAD then names exactly what was checked.
if [[ -z "$(git status --porcelain)" ]]; then
  tree=$(git rev-parse 'HEAD^{tree}')
fi

stamp_key() {
  case "$1" in
    web*) node_v="${node_v:-$(node -v 2>/dev/null || echo none)}" && echo "$tree node-$node_v $1" ;;
    *) rust_tc="${rust_tc:-$(rustc -V | tr ' ' '_')}" && echo "$tree $rust_tc $1" ;;
  esac
}

# passed <phase>: the phase passed on this tree before.
passed() {
  [[ -n "$tree" ]] && ((!force)) && [[ -f "$stamp_file" ]] && grep -qxF "$(stamp_key "$1")" "$stamp_file"
}

stamp() {
  [[ -n "$tree" ]] || return 0
  stamp_key "$1" >>"$stamp_file"
  if (($(wc -l <"$stamp_file") > 5000)); then
    tail -n 2000 "$stamp_file" >"$stamp_file.tmp" && mv "$stamp_file.tmp" "$stamp_file"
  fi
}

# --- Phases ------------------------------------------------------------------------------------------------------

# phase <name> <stamp|-> <command...>: run the command and record its seconds; skip it when this tree passed the
# stamp (the web tests of a diff also when it passed the full web check).
phase() {
  local name="$1" key="$2" start=$SECONDS
  shift 2
  if [[ "$key" != - ]] && { passed "$key" || { [[ "$key" == web-changed* ]] && passed web; }; }; then
    echo "$name: skipped: tree already passed"
    return
  fi
  "$@"
  secs[$name]=$((SECONDS - start))
  [[ "$key" == - ]] || stamp "$key"
}

web_checks() {
  local tests=(pnpm --silent test)
  [[ "$1" == changed ]] && tests=(pnpm exec vitest run --changed "$base" --passWithNoTests)
  (cd web && pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format && "${tests[@]}")
}

# take_slot: hold one of the machine-wide test slots on fd 8 (the first free one, else wait on slot 1 and look again).
take_slot() {
  local start=$SECONDS i
  if command -v flock >/dev/null; then
    while true; do
      for ((i = 1; i <= slots; i++)); do
        exec 8>"/tmp/mtg-test-slot.$i"
        flock -n 8 && break 2
      done
      exec 8>"/tmp/mtg-test-slot.1"
      flock -w 5 8 && break
    done
  fi
  secs["slot wait"]=$((${secs["slot wait"]:-0} + SECONDS - start))
}

# run_tests <phase> <cargo test args...>: run the tests in a slot under the wall-clock limit and keep the output.
run_tests() {
  local name="$1" start rc=0 limit=() limit_secs
  shift
  limit_secs=$(awk -v m="$test_timeout" 'BEGIN { printf "%d", m * 60 }')
  command -v timeout >/dev/null && limit=(timeout --signal=KILL "${test_timeout}m")
  take_slot
  run_log=$(mktemp "${TMPDIR:-/tmp}/mtg-test-run.XXXXXX")
  start=$SECONDS
  # Fd 8 is closed for the children, so a test left behind never keeps the slot.
  "${limit[@]}" cargo test "$@" --quiet 8>&- 2>&1 | tee "$run_log" || rc=${PIPESTATUS[0]}
  secs[$name]=$((SECONDS - start))
  exec 8>&-
  if ((rc == 124 || rc == 137)) && ((SECONDS - start >= limit_secs - 2)); then
    echo "test run exceeded $test_timeout minutes: a hung test"
    echo "running for over 60 seconds when it was killed:"
    sed -n 's/.*test \([^ ]*\) has been running for over 60 seconds.*/  \1/p' "$run_log" | sort -u
    exit 1
  fi
  rm -f "$run_log"
  run_log=""
  ((rc == 0)) || exit "$rc"
}

# pkg_args <crate...>: `-p` arguments for cargo.
pkg_args() {
  local name
  for name in "$@"; do printf -- '-p\n%s\n' "$name"; done
}

# --- Plan --------------------------------------------------------------------------------------------------------

other_sel=() pg_sel=()
if [[ "$mode" != web ]]; then
  load_graph
fi
case "$mode" in
  rust | all) for name in "${all_crates[@]}"; do hit[$name]=1; done ;;
  affected)
    base="${revs[0]:-$(git merge-base HEAD origin/main 2>/dev/null || git merge-base HEAD main)}"
    base=$(git rev-parse --verify "$base^{commit}")
    head_rev="${revs[1]:-}"
    [[ -z "$head_rev" ]] || tree=""
    nfiles=0
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
    close_over_dependents
    ;;
esac
[[ "$mode" == all || "$mode" == web ]] && web_sel=full
if [[ "$mode" != web ]]; then
  for name in "${all_crates[@]}"; do
    [[ -v hit[$name] ]] || continue
    if passed "test:$name"; then
      echo "tests of $name: skipped: tree already passed"
      continue
    fi
    if [[ "$pg_crates" == *" $name "* ]]; then pg_sel+=("$name"); else other_sel+=("$name"); fi
  done
fi
[[ -d web ]] || web_sel=none

if ((list)); then
  echo "base: $base"
  echo "changed files: $nfiles"
  echo "fmt"
  echo "clippy"
  echo "script tests"
  echo "tests without Postgres: ${other_sel[*]:--}"
  echo "tests with Postgres: ${pg_sel[*]:--}"
  echo "web: $web_sel"
  exit 0
fi

# --- Run ---------------------------------------------------------------------------------------------------------

if [[ "$mode" != web ]]; then
  phase "fmt" fmt cargo fmt --all --check
  phase "clippy" clippy cargo clippy --workspace --all-targets --quiet -- -D warnings
  phase "script tests" script-tests scripts/tests/check_select.sh
fi

mapfile -t other_pkgs < <(pkg_args "${other_sel[@]}")
mapfile -t pg_pkgs < <(pkg_args "${pg_sel[@]}")

build_tests() {
  ((${#other_pkgs[@]} == 0)) || cargo test "${other_pkgs[@]}" --quiet --no-run
  ((${#pg_pkgs[@]} == 0)) || cargo test "${pg_pkgs[@]}" --quiet --no-run
}

# stamp_crates <crate...>: record that the tests of these crates passed.
stamp_crates() {
  local name
  for name in "$@"; do stamp "test:$name"; done
}

pg_tests() {
  run_tests "tests with Postgres" "${pg_pkgs[@]}"
  stamp_crates "${pg_sel[@]}"
}

if ((${#other_pkgs[@]} + ${#pg_pkgs[@]})); then
  # Compile first: the lock only has to cover the Postgres run, not the slow optimised test builds.
  phase "test build" - build_tests
fi

if ((${#pg_pkgs[@]})); then
  # Reuse a Postgres that is already listening (e.g. another worktree started one).
  if command -v docker >/dev/null && [[ -f docker-compose.yml ]] && ! (exec 3<>/dev/tcp/127.0.0.1/5432) 2>/dev/null; then
    docker compose up -d --wait db >/dev/null
  fi
  export DATABASE_URL="${DATABASE_URL:-postgres://magic:magic@localhost:5432/magic}"
  # sqlx::test drops other runs' test databases on a shared server, so one Postgres test run at a time.
  # Take the lock at once when it is free; when it is busy, run the other crates first and wait afterwards.
  have_lock=0 have_flock=0
  if command -v flock >/dev/null; then
    have_flock=1
    exec 9>"${TMPDIR:-/tmp}/mtg-pg-tests.lock"
    flock -n 9 && have_lock=1
  fi
  if ((have_lock)); then
    pg_tests
    exec 9>&-
  fi
fi
if ((${#other_pkgs[@]})); then
  run_tests "tests without Postgres" "${other_pkgs[@]}"
  stamp_crates "${other_sel[@]}"
fi
if ((${#pg_pkgs[@]})) && ((!have_lock)); then
  if ((have_flock)); then
    lock_start=$SECONDS
    flock 9
    secs["lock wait"]=$((SECONDS - lock_start))
  fi
  pg_tests
  exec 9>&-
fi

if [[ "$web_sel" != none ]]; then
  web_key=web
  [[ "$web_sel" == full ]] || web_key="web-changed:$base"
  phase "web" "$web_key" web_checks "$web_sel"
fi

for name in "${phases[@]}"; do
  [[ -v secs[$name] ]] && echo "$name: ${secs[$name]}s"
done
echo "check ($mode): ok"
