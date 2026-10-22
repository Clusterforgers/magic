#!/usr/bin/env bash
# Usage: scripts/check.sh [fast|rust|web|all]   (default: all)
#   fast: fmt + clippy   rust: fast + cargo tests   web: web checks only   all: everything
# The last lines give the seconds each phase took and the wait for the Postgres lock.
set -euo pipefail
cd "$(dirname "$0")/.."

mode="${1:-all}"
phases=("fmt" "clippy" "test build" "tests without Postgres" "tests with Postgres" "lock wait" "web")
declare -A secs

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

# Only these crates connect to Postgres (`sqlx::test`); the others run without the database or the lock.
pg_pkgs=(-p mtg-db -p mtg-server -p mtg-cli)
other_pkgs=(--workspace --exclude mtg-db --exclude mtg-server --exclude mtg-cli)

build_tests() {
  cargo test "${other_pkgs[@]}" --quiet --no-run
  cargo test "${pg_pkgs[@]}" --quiet --no-run
}

pg_tests() {
  phase "tests with Postgres" cargo test "${pg_pkgs[@]}" --quiet
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
  phase "tests without Postgres" cargo test "${other_pkgs[@]}" --quiet
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
fi

if [[ "$mode" == web || "$mode" == all ]] && [[ -d web ]]; then
  phase "web" web_checks
fi

for name in "${phases[@]}"; do
  [[ -v secs[$name] ]] && echo "$name: ${secs[$name]}s"
done
echo "check ($mode): ok"
