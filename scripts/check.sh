#!/usr/bin/env bash
# Usage: scripts/check.sh [fast|rust|web|all]   (default: all)
#   fast: fmt + clippy   rust: fast + cargo tests   web: web checks only   all: everything
set -euo pipefail
cd "$(dirname "$0")/.."

mode="${1:-all}"

if [[ "$mode" != web ]]; then
  cargo fmt --all --check
  cargo clippy --workspace --all-targets --quiet -- -D warnings
fi

if [[ "$mode" == rust || "$mode" == all ]]; then
  # Compile first: the lock below only has to cover the run, not the slow optimised test builds.
  cargo test --workspace --quiet --no-run
  # Reuse a Postgres that is already listening (e.g. another worktree started one).
  if command -v docker >/dev/null && [[ -f docker-compose.yml ]] && ! (exec 3<>/dev/tcp/127.0.0.1/5432) 2>/dev/null; then
    docker compose up -d --wait db >/dev/null
  fi
  export DATABASE_URL="${DATABASE_URL:-postgres://magic:magic@localhost:5432/magic}"
  # sqlx::test drops other runs' test databases on a shared server, so one test run at a time uses it.
  if command -v flock >/dev/null; then
    flock "${TMPDIR:-/tmp}/mtg-pg-tests.lock" cargo test --workspace --quiet
  else
    cargo test --workspace --quiet
  fi
fi

if [[ "$mode" == web || "$mode" == all ]] && [[ -d web ]]; then
  (cd web && pnpm --silent typecheck && pnpm --silent lint && pnpm --silent format && pnpm --silent test)
fi

echo "check ($mode): ok"
