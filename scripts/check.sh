#!/usr/bin/env bash
# Usage: scripts/check.sh [fast|rust|web|all]   (default: all)
#   fast: fmt + clippy   rust: fast + cargo tests   web: web checks only   all: everything
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

mode="${1:-all}"
phases=("fmt" "clippy" "test build" "tests without Postgres" "tests with Postgres" "lock wait" "slot wait" "web")
declare -A secs
run_log=""
trap '[[ -z "$run_log" ]] || rm -f "$run_log"' EXIT

# phase <name> <command...>: run the command and record its seconds.
phase() {
  local name="$1" start=$SECONDS
  shift
  "$@"
  secs[$name]=$((SECONDS - start))
}

web_checks() {
  (cd web && pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format && pnpm --silent test)
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

# Only these crates connect to Postgres (`sqlx::test`); the others run without the database or the lock.
pg_pkgs=(-p mtg-db -p mtg-server -p mtg-cli)
other_pkgs=(--workspace --exclude mtg-db --exclude mtg-server --exclude mtg-cli)

build_tests() {
  cargo test "${other_pkgs[@]}" --quiet --no-run
  cargo test "${pg_pkgs[@]}" --quiet --no-run
}

pg_tests() {
  run_tests "tests with Postgres" "${pg_pkgs[@]}"
}

if [[ "$mode" != web ]]; then
  phase "fmt" cargo fmt --all --check
  phase "clippy" cargo clippy --workspace --all-targets --quiet -- -D warnings
fi

if [[ "$mode" == rust || "$mode" == all ]]; then
  # Compile first: the lock only has to cover the Postgres run, not the slow optimised test builds.
  phase "test build" build_tests
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
  run_tests "tests without Postgres" "${other_pkgs[@]}"
  if ((!have_lock)); then
    if ((have_flock)); then
      lock_start=$SECONDS
      flock 9
      secs["lock wait"]=$((SECONDS - lock_start))
    fi
    pg_tests
    exec 9>&-
  fi
  secs["lock wait"]=${secs["lock wait"]:-0}
  secs["slot wait"]=${secs["slot wait"]:-0}
fi

if [[ "$mode" == web || "$mode" == all ]] && [[ -d web ]]; then
  phase "web" web_checks
fi

for name in "${phases[@]}"; do
  [[ -v secs[$name] ]] && echo "$name: ${secs[$name]}s"
done
echo "check ($mode): ok"
