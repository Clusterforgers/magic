#!/usr/bin/env bash
# Usage: scripts/check.sh [fast|rust|web|all]   (default: all)
#   fast: fmt + clippy   rust: fast + cargo tests   web: web checks only   all: everything
# The last lines give the seconds each phase took and the wait for the Postgres lock.
set -euo pipefail
cd "$(dirname "$0")/.."

mode="${1:-all}"
laps=()

# phase <name> <command...>: run the command and record its seconds.
phase() {
  local name="$1" start=$SECONDS
  shift
  "$@"
  laps+=("$name: $((SECONDS - start))s")
}

web_checks() {
  (cd web && pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format && pnpm --silent test)
}

if [[ "$mode" != web ]]; then
  phase fmt cargo fmt --all --check
  phase clippy cargo clippy --workspace --all-targets --quiet -- -D warnings
fi

if [[ "$mode" == rust || "$mode" == all ]]; then
  # Only these crates connect to Postgres (`sqlx::test`); the others run without the database or the lock.
  pg_crates=(-p mtg-db -p mtg-server -p mtg-cli)
  # Compile first: the lock below only has to cover the run, not the slow optimised test builds.
  phase "test build" cargo test --workspace --quiet --no-run
  phase "tests without Postgres" cargo test --workspace --exclude mtg-db --exclude mtg-server --exclude mtg-cli --quiet
  # Reuse a Postgres that is already listening (e.g. another worktree started one).
  if command -v docker >/dev/null && [[ -f docker-compose.yml ]] && ! (exec 3<>/dev/tcp/127.0.0.1/5432) 2>/dev/null; then
    docker compose up -d --wait db >/dev/null
  fi
  export DATABASE_URL="${DATABASE_URL:-postgres://magic:magic@localhost:5432/magic}"
  # sqlx::test drops other runs' test databases on a shared server, so one test run at a time uses it.
  lock_wait=0
  if command -v flock >/dev/null; then
    lock_start=$SECONDS
    exec 9>"${TMPDIR:-/tmp}/mtg-pg-tests.lock"
    flock 9
    lock_wait=$((SECONDS - lock_start))
  fi
  phase "tests with Postgres" cargo test "${pg_crates[@]}" --quiet
  exec 9>&-
  laps+=("lock wait: ${lock_wait}s")
fi

if [[ "$mode" == web || "$mode" == all ]] && [[ -d web ]]; then
  phase web web_checks
fi

printf '%s\n' "${laps[@]}"
echo "check ($mode): ok"
